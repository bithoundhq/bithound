//! End-to-end smoke test for V0.
//!
//! Spawns the `bithound` binary against a mock bitcoind RPC server and
//! a mock webhook receiver, drives the `bitcoin.tip_lag_or_ibd_stalled`
//! A1 pattern through the entire pipeline, and asserts the webhook
//! sees an `Opened` lifecycle event with the right kind, severity, and
//! subject.
//!
//! This is `#[ignore]`-gated so `cargo test` in CI doesn't spin a
//! bithound subprocess on every run. Enable with one of:
//!
//! * `cargo test --ignored --test e2e_tip_lag`
//! * `BITHOUND_TEST_REGTEST=1 cargo test --test e2e_tip_lag -- --ignored`
//!
//! The test uses a hand-rolled JSON-RPC mock server that holds the A1
//! preconditions constant across polls. The collector polls every
//! second; the rule debounces over two consecutive ticks; so the
//! webhook fires within ~3 seconds. Total deadline is 30 seconds to
//! allow for cold-start of the sqlx pool and the supervisor's first
//! tick.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::time::timeout;

// ─── A1 fixtures ──────────────────────────────────────────────────────
// Held constant across every poll so the rule sees two consecutive
// firing ticks and lifts the draft into an Opened incident.

fn a1_blockchain() -> serde_json::Value {
    // gap = 500 (< 1000), verificationprogress > 0.999, IBD true.
    serde_json::json!({
        "chain": "main",
        "blocks": 899_500,
        "headers": 900_000,
        "bestblockhash": "deadbeef",
        "verificationprogress": 0.99996,
        "initialblockdownload": true,
        "pruned": false,
        "size_on_disk": 500_000_000_000u64,
    })
}

fn a1_mempool() -> serde_json::Value {
    serde_json::json!({
        "loaded": true,
        "size": 0,
        "bytes": 0,
        "usage": 0,
        "maxmempool": 300_000_000,
    })
}

fn a1_network() -> serde_json::Value {
    serde_json::json!({
        "version": 250_000,
        "subversion": "/Satoshi:25.0.0/",
        "protocolversion": 70_016,
        "connections": 10,
        "connections_in": 0,
        "connections_out": 10,
        "networkactive": true,
    })
}

fn a1_peers() -> serde_json::Value {
    // 10 entries — clears the rule's >= 8 floor.
    let entries: Vec<serde_json::Value> = (0..10)
        .map(|i| {
            serde_json::json!({
                "inbound": false,
                "addr": format!("10.0.0.{}:8333", i + 1),
                "subver": "/Satoshi:25.0.0/",
                "connection_type": "outbound-full-relay",
            })
        })
        .collect();
    serde_json::Value::Array(entries)
}

// ─── Mock servers ─────────────────────────────────────────────────────

/// Spawn a hand-rolled JSON-RPC server that dispatches on the request
/// `method` field and replies with the corresponding A1 fixture. The
/// `id` field of the request is echoed into the response so the
/// collector's id-correlation check passes.
async fn spawn_mock_bitcoind() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = match listener.accept().await {
                Ok(pair) => pair,
                Err(_) => return,
            };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 16 * 1024];
                let n = match socket.read(&mut buf).await {
                    Ok(0) | Err(_) => return,
                    Ok(n) => n,
                };
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let body_start = req.find("\r\n\r\n").map(|i| i + 4).unwrap_or(req.len());
                let parsed = serde_json::from_str::<serde_json::Value>(&req[body_start..])
                    .unwrap_or(serde_json::Value::Null);
                let method = parsed
                    .get("method")
                    .and_then(|m| m.as_str())
                    .unwrap_or("")
                    .to_string();
                let request_id = parsed.get("id").cloned().unwrap_or(serde_json::Value::Null);

                let result = match method.as_str() {
                    "getblockchaininfo" => a1_blockchain(),
                    "getmempoolinfo" => a1_mempool(),
                    "getnetworkinfo" => a1_network(),
                    "getpeerinfo" => a1_peers(),
                    _ => serde_json::json!(null),
                };
                let response_body = serde_json::json!({
                    "id": request_id,
                    "result": result,
                    "error": serde_json::Value::Null,
                });
                let body_bytes = serde_json::to_vec(&response_body).expect("encode");
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body_bytes.len()
                );
                let _ = socket.write_all(header.as_bytes()).await;
                let _ = socket.write_all(&body_bytes).await;
                let _ = socket.shutdown().await;
            });
        }
    });
    addr
}

/// Spawn a hand-rolled HTTP server that captures every POST body as
/// parsed JSON. Returns the bound address and a shared handle to the
/// captured payload list.
async fn spawn_mock_webhook() -> (SocketAddr, Arc<Mutex<Vec<serde_json::Value>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let captured: Arc<Mutex<Vec<serde_json::Value>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_for_task = Arc::clone(&captured);
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = match listener.accept().await {
                Ok(pair) => pair,
                Err(_) => return,
            };
            let captured = Arc::clone(&captured_for_task);
            tokio::spawn(async move {
                let mut buf = vec![0u8; 64 * 1024];
                let n = match socket.read(&mut buf).await {
                    Ok(0) | Err(_) => return,
                    Ok(n) => n,
                };
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let body_start = req.find("\r\n\r\n").map(|i| i + 4).unwrap_or(req.len());
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&req[body_start..]) {
                    captured.lock().await.push(value);
                }
                let header = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = socket.write_all(header.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });
    (addr, captured)
}

// ─── Config wiring ────────────────────────────────────────────────────

/// Write a temp `bithound.toml` pointing at the supplied mock
/// endpoints. The sidecar id and SQLite database go in the same
/// tempdir so the test is fully self-contained.
fn write_config(tempdir: &Path, bitcoind: SocketAddr) -> PathBuf {
    let id_file = tempdir.join("sidecar_id");
    let db_path = tempdir.join("bithound.db");
    let config_path = tempdir.join("bithound.toml");

    let toml = format!(
        r#"
[sidecar]
id_file = "{id}"
log_level = "info"

[storage]
db_path = "{db}"

[runtime]
channel_capacity = 64
shutdown_deadline_seconds = 5

[[bitcoin_nodes]]
id = "btc-test"
rpc_url = "http://{bitcoind}"

[bitcoin_nodes.auth]
type = "user_pass"
user = "test"
password_env = "BITHOUND_E2E_BITCOIND_PASSWORD"

[[collectors]]
id = "btc-test-rpc"
target = {{ type = "bitcoin_node", id = "btc-test" }}
integration = {{ type = "bitcoin_core_rpc", interval_seconds = 1 }}
instance_label = "test"
description = "e2e mock bitcoind"

[[notification_rules]]
id = "all-to-webhook"
name = "All events to webhook"
enabled = true
min_severity = "info"
event_kinds = []

[notification_rules.target]
type = "webhook"
url_env = "BITHOUND_E2E_WEBHOOK_URL"
"#,
        id = id_file.display(),
        db = db_path.display(),
        bitcoind = bitcoind,
    );
    std::fs::write(&config_path, toml).expect("write config");
    config_path
}

// ─── The test ─────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "spawns bithound binary + bitcoind mock; opt in with `cargo test --ignored` or `BITHOUND_TEST_REGTEST=1 cargo test`"]
async fn e2e_tip_lag_or_ibd_stalled_fires_via_webhook() {
    let bitcoind = spawn_mock_bitcoind().await;
    let (webhook, captured) = spawn_mock_webhook().await;
    let tempdir = tempfile::tempdir().expect("tempdir");
    let config_path = write_config(tempdir.path(), bitcoind);

    let bithound_exe = env!("CARGO_BIN_EXE_bithound");
    let mut child = tokio::process::Command::new(bithound_exe)
        .arg("--config")
        .arg(&config_path)
        .env("BITHOUND_E2E_BITCOIND_PASSWORD", "test-password")
        .env(
            "BITHOUND_E2E_WEBHOOK_URL",
            format!("http://{}/incident", webhook),
        )
        .env("RUST_LOG", "info")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn bithound");

    // Drain bithound's stdout/stderr in background tasks. The
    // tokio::process::Command pipe buffer is small (~64 KB on macOS);
    // at info log level the runtime fills it within a few polls and
    // blocks the child if we don't keep reading. Both buffers are
    // surfaced into the panic message if the timeout fires.
    let stdout_buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let stderr_buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    if let Some(stdout) = child.stdout.take() {
        let buf = Arc::clone(&stdout_buf);
        tokio::spawn(async move {
            let mut s = stdout;
            let mut tmp = String::new();
            let _ = s.read_to_string(&mut tmp).await;
            buf.lock().await.push_str(&tmp);
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let buf = Arc::clone(&stderr_buf);
        tokio::spawn(async move {
            let mut s = stderr;
            let mut tmp = String::new();
            let _ = s.read_to_string(&mut tmp).await;
            buf.lock().await.push_str(&tmp);
        });
    }

    // The runtime's notification worker (see
    // `src/runtime/notification_worker.rs::NotifierSenders::dispatch`)
    // serializes a webhook POST with `title`, `summary`,
    // `affected_component`, `diagnostic_summary`, `occurred_at`. The
    // title is the canonical place to read the lifecycle kind +
    // severity + incident kind out of the payload, in the shape
    // `"<KIND> [<Severity>] <incident_kind>"` (e.g.
    // `"OPENED [Critical] bitcoin.tip_lag_or_ibd_stalled"`).
    let received = timeout(Duration::from_secs(20), async {
        loop {
            tokio::time::sleep(Duration::from_millis(250)).await;
            let guard = captured.lock().await;
            if let Some(body) = guard
                .iter()
                .find(|b| {
                    let title = b.get("title").and_then(|v| v.as_str()).unwrap_or("");
                    title.starts_with("OPENED ") && title.contains("bitcoin.tip_lag_or_ibd_stalled")
                })
                .cloned()
            {
                return body;
            }
        }
    })
    .await;

    // Capture stdout/stderr on failure so the operator can diagnose
    // why the rule never fired. `kill_on_drop` would tear the child
    // down silently otherwise.
    let kill = child.kill().await;

    let body = match received {
        Ok(body) => body,
        Err(_) => {
            let _ = kill;
            // Give the stdout/stderr drain tasks a moment to flush
            // anything bithound emitted on its way out.
            tokio::time::sleep(Duration::from_millis(250)).await;
            let stdout = stdout_buf.lock().await.clone();
            let stderr = stderr_buf.lock().await.clone();
            panic!(
                "webhook never received Opened bitcoin.tip_lag_or_ibd_stalled within 20s.\n\
                 captured POSTs ({n}):\n{captured}\n\
                 ---\nbithound stdout:\n{stdout}\n\
                 ---\nbithound stderr:\n{stderr}",
                n = captured.lock().await.len(),
                captured = captured
                    .lock()
                    .await
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
                stdout = stdout,
                stderr = stderr,
            );
        }
    };

    let title = body["title"].as_str().expect("title should be a string");
    assert!(
        title.starts_with("OPENED "),
        "title should begin with the lifecycle kind, got {title:?}",
    );
    assert!(
        title.contains("[Critical]"),
        "title should embed the incident severity, got {title:?}",
    );
    assert!(
        title.contains("bitcoin.tip_lag_or_ibd_stalled"),
        "title should embed the incident kind, got {title:?}",
    );
    assert!(
        body["summary"]
            .as_str()
            .map(|s| s.contains("btc-test"))
            .unwrap_or(false),
        "summary should reference the configured node id, got {}",
        body["summary"]
    );
    assert!(
        body["occurred_at"].is_string(),
        "occurred_at should be a RFC3339 timestamp string, got {}",
        body["occurred_at"]
    );
}
