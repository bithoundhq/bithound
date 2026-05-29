//! End-to-end test for catalog entry B1 (channel inactive — peer offline)
//! against a real LND running inside a Polar regtest network.
//!
//! This test is `#[ignore]`-gated because it needs a running Polar
//! network and the operator to take a peer down by hand. CI doesn't
//! have Docker, so the test only fires when an operator opts in:
//!
//! ```text
//! cargo test --test e2e_lnd_b1_via_polar -- --ignored --nocapture
//! ```
//!
//! `tests/POLAR.md` is the setup procedure: install Polar, open a
//! channel between two LND nodes, and export the five env vars the
//! test reads:
//!
//! - `BITHOUND_TEST_POLAR_LND_GRPC`
//! - `BITHOUND_TEST_POLAR_LND_CERT`
//! - `BITHOUND_TEST_POLAR_LND_MACAROON_HEX`
//! - `BITHOUND_TEST_POLAR_BITCOIN_RPC`
//! - `BITHOUND_TEST_POLAR_BITCOIN_USER` + `_PASS`
//!
//! ## Why "scaffold" and not "automated"
//!
//! The test spawns bithound against Polar and listens on a webhook
//! port, but Polar's docker stack doesn't expose a stable API for
//! "stop a node" the way Polar's UI does (right-click → Stop). The
//! channel-down trigger is therefore manual: the operator pauses
//! the peer node in Polar's UI after the test prints the
//! "waiting for channel to go inactive…" line. The test asserts the
//! webhook receives the right shape within a long-but-bounded
//! deadline.

use std::env;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio::time::timeout;

/// Names of every env var the test reads, in one place so the
/// "missing env var, skipping" message can enumerate them.
const REQUIRED_ENV_VARS: &[&str] = &[
    "BITHOUND_TEST_POLAR_LND_GRPC",
    "BITHOUND_TEST_POLAR_LND_CERT",
    "BITHOUND_TEST_POLAR_LND_MACAROON_HEX",
    "BITHOUND_TEST_POLAR_BITCOIN_RPC",
    "BITHOUND_TEST_POLAR_BITCOIN_USER",
    "BITHOUND_TEST_POLAR_BITCOIN_PASS",
];

fn polar_env() -> Result<PolarEnv, String> {
    let missing: Vec<&str> = REQUIRED_ENV_VARS
        .iter()
        .copied()
        .filter(|k| env::var(k).is_err())
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "missing env var(s): {}. See tests/POLAR.md.",
            missing.join(", ")
        ));
    }
    Ok(PolarEnv {
        lnd_grpc: env::var("BITHOUND_TEST_POLAR_LND_GRPC").unwrap(),
        lnd_cert: env::var("BITHOUND_TEST_POLAR_LND_CERT").unwrap(),
        lnd_macaroon_hex: env::var("BITHOUND_TEST_POLAR_LND_MACAROON_HEX").unwrap(),
        bitcoin_rpc: env::var("BITHOUND_TEST_POLAR_BITCOIN_RPC").unwrap(),
        bitcoin_user: env::var("BITHOUND_TEST_POLAR_BITCOIN_USER").unwrap(),
        bitcoin_pass: env::var("BITHOUND_TEST_POLAR_BITCOIN_PASS").unwrap(),
    })
}

struct PolarEnv {
    lnd_grpc: String,
    lnd_cert: String,
    lnd_macaroon_hex: String,
    bitcoin_rpc: String,
    bitcoin_user: String,
    bitcoin_pass: String,
}

#[tokio::test]
#[ignore = "needs a running Polar network; see tests/POLAR.md and BITHOUND_TEST_POLAR_* env vars"]
async fn b1_channel_inactive_fires_via_polar() {
    let polar = match polar_env() {
        Ok(p) => p,
        Err(reason) => {
            eprintln!("skipping: {reason}");
            return;
        }
    };

    // Webhook receiver on a free port. The test asserts a POST body
    // containing the lnd.channel_inactive kind lands here within the
    // deadline below.
    let webhook_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind webhook listener");
    let webhook_addr = webhook_listener
        .local_addr()
        .expect("webhook listener local_addr");
    let webhook_url = format!("http://{webhook_addr}/lnd-channel-inactive");
    let received: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));

    // Spawn the webhook acceptor task. It echoes back a 204 and
    // records the request body — bithound's webhook sender doesn't
    // care about the response body, only the status code.
    let received_for_task = Arc::clone(&received);
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = webhook_listener.accept().await else {
                return;
            };
            let received = Arc::clone(&received_for_task);
            tokio::spawn(async move {
                let mut buf = vec![0u8; 32 * 1024];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                received
                    .lock()
                    .await
                    .push(String::from_utf8_lossy(&buf[..n]).into_owned());
                let _ = sock
                    .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n")
                    .await;
                let _ = sock.shutdown().await;
            });
        }
    });

    // Build a bithound config that points at Polar's LND + bitcoind
    // and the webhook above. The config lives in a tempdir alongside
    // the SQLite db so the test doesn't pollute the host.
    let workdir = tempfile::tempdir().expect("tempdir");
    let config_path = workdir.path().join("bithound.toml");
    let db_path = workdir.path().join("bithound.db");
    let id_file = workdir.path().join("sidecar_id");

    let toml = render_config(&polar, &webhook_url, &db_path, &id_file);
    std::fs::write(&config_path, toml).expect("write config");

    // Find the bithound binary cargo built. Cargo sets `CARGO` in
    // tests, but for the binary itself we rely on the conventional
    // target/<profile>/bithound layout.
    let bithound_path = find_bithound_binary();

    let mut child = tokio::process::Command::new(&bithound_path)
        .arg("--config")
        .arg(&config_path)
        .env("BITHOUND_LND_TEST_MACAROON", &polar.lnd_macaroon_hex)
        .env("BITHOUND_LND_TEST_WEBHOOK_URL", &webhook_url)
        .env("BITHOUND_BITCOIN_TEST_PASS", &polar.bitcoin_pass)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn bithound");

    eprintln!(
        "bithound spawned (pid={}). Open Polar and STOP the peer LND \
         node on the channel under test. Test waits up to 12 minutes.",
        child.id().unwrap_or(0)
    );

    // Public channels: 5-minute debounce. Private: 30. Give the
    // operator + the rule enough margin: 12 minutes total.
    let deadline = Duration::from_secs(12 * 60);
    let event_seen = timeout(deadline, async {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            let bodies = received.lock().await;
            if bodies.iter().any(|b| b.contains("lnd.channel_inactive")) {
                return true;
            }
        }
    })
    .await;

    // Always tear bithound down before asserting so a failing
    // assertion doesn't leak a daemon.
    let _ = child.kill().await;
    let _ = child.wait().await;

    match event_seen {
        Ok(true) => { /* pass */ }
        Ok(false) => unreachable!("loop only returns true"),
        Err(_) => panic!(
            "no lnd.channel_inactive event within 12 minutes. \
             Did you stop the peer in Polar's UI? Captured webhook \
             bodies so far: {:?}",
            received.lock().await
        ),
    }
}

/// Render a minimal bithound config pointing at Polar's LND +
/// bitcoind and our local webhook receiver.
fn render_config(
    polar: &PolarEnv,
    webhook_url: &str,
    db_path: &std::path::Path,
    id_file: &std::path::Path,
) -> String {
    let _ = webhook_url; // referenced through BITHOUND_LND_TEST_WEBHOOK_URL env var
    format!(
        r#"[sidecar]
id_file = "{id_file}"
log_level = "bithound=debug,sqlx=warn"

[storage]
db_path = "{db_path}"

[runtime]
channel_capacity = 256
shutdown_deadline_seconds = 10

[api]
enabled = false

[[bitcoin_nodes]]
id = "polar-btc"
rpc_url = "{rpc_url}"

[bitcoin_nodes.auth]
type = "user_pass"
user = "{user}"
password_env = "BITHOUND_BITCOIN_TEST_PASS"

[[lnd_nodes]]
id = "polar-lnd"
grpc_endpoint = "{lnd_grpc}"
macaroon_env = "BITHOUND_LND_TEST_MACAROON"
tls_cert_path = "{lnd_cert}"

[[collectors]]
id = "polar-btc-rpc"
target = {{ type = "bitcoin_node", id = "polar-btc" }}
integration = {{ type = "bitcoin_core_rpc", interval_seconds = 5 }}
instance_label = "polar-btc"

[[collectors]]
id = "polar-lnd-grpc"
target = {{ type = "lnd_node", id = "polar-lnd" }}
integration = {{ type = "lnd_grpc_poll", interval_seconds = 5 }}
instance_label = "polar-lnd"

[[notification_rules]]
id = "lnd-channel-inactive-to-webhook"
name = "lnd.channel_inactive -> webhook"
enabled = true
min_severity = "warning"
event_kinds = ["lnd.channel_inactive"]

[notification_rules.target]
type = "webhook"
url_env = "BITHOUND_LND_TEST_WEBHOOK_URL"
"#,
        id_file = id_file.display(),
        db_path = db_path.display(),
        rpc_url = polar.bitcoin_rpc,
        user = polar.bitcoin_user,
        lnd_grpc = polar.lnd_grpc,
        lnd_cert = polar.lnd_cert,
    )
}

/// Locate the bithound binary cargo just built. The integration test
/// runner sets `CARGO_BIN_EXE_<name>` for binaries declared in the
/// crate's `[[bin]]` section; bithound's binary name matches the
/// crate name so the env var is `CARGO_BIN_EXE_bithound`.
fn find_bithound_binary() -> PathBuf {
    env::var_os("CARGO_BIN_EXE_bithound")
        .map(PathBuf::from)
        .expect(
            "CARGO_BIN_EXE_bithound not set. \
             Run via `cargo test --test e2e_lnd_b1_via_polar`.",
        )
}
