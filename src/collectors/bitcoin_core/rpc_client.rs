//! Thin JSON-RPC wrapper over `reqwest::Client` for the four Bitcoin
//! Core RPCs the V0 collector needs. Hand-rolled rather than depending
//! on a published RPC crate because the surface is tiny.

use std::time::Duration;

use reqwest::StatusCode;
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::time::error::Elapsed;

use crate::collectors::registry::BitcoinRpcAuth;
use crate::collectors::CollectionErrorKind;

#[derive(Debug, Clone)]
pub struct BitcoinRpcClient {
    url: String,
    auth: BitcoinRpcAuth,
    http: reqwest::Client,
    timeout: Duration,
}

impl BitcoinRpcClient {
    pub fn new(
        url: String,
        auth: BitcoinRpcAuth,
        http: reqwest::Client,
        timeout: Duration,
    ) -> Self {
        Self {
            url,
            auth,
            http,
            timeout,
        }
    }

    pub async fn get_blockchain_info(&self) -> Result<GetBlockchainInfoResponse, RpcError> {
        self.call("getblockchaininfo", serde_json::json!([])).await
    }

    pub async fn get_mempool_info(&self) -> Result<GetMempoolInfoResponse, RpcError> {
        self.call("getmempoolinfo", serde_json::json!([])).await
    }

    pub async fn get_network_info(&self) -> Result<GetNetworkInfoResponse, RpcError> {
        self.call("getnetworkinfo", serde_json::json!([])).await
    }

    pub async fn get_peer_info(&self) -> Result<GetPeerInfoResponse, RpcError> {
        self.call("getpeerinfo", serde_json::json!([])).await
    }

    async fn call<T: for<'de> Deserialize<'de>>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T, RpcError> {
        let envelope = RpcRequest {
            jsonrpc: "1.0",
            id: "bithound",
            method,
            params,
        };

        let mut builder = self.http.post(&self.url).json(&envelope);

        let (user, pass) = resolve_auth(&self.auth)?;
        builder = builder.basic_auth(user, Some(pass));

        let fut = builder.send();
        let response = tokio::time::timeout(self.timeout, fut)
            .await
            .map_err(|_: Elapsed| RpcError::Timeout)??;

        let status = response.status();
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Err(RpcError::Auth);
        }
        if !status.is_success() {
            return Err(RpcError::HttpStatus(status.as_u16()));
        }

        let body_fut = response.bytes();
        let body = tokio::time::timeout(self.timeout, body_fut)
            .await
            .map_err(|_: Elapsed| RpcError::Timeout)??;

        let envelope: RpcResponse<T> = serde_json::from_slice(&body)?;
        if let Some(err) = envelope.error {
            return Err(RpcError::BitcoindError {
                code: err.code,
                message: err.message,
            });
        }
        envelope.result.ok_or(RpcError::BitcoindError {
            code: 0,
            message: "missing result in JSON-RPC envelope".into(),
        })
    }
}

fn resolve_auth(auth: &BitcoinRpcAuth) -> Result<(String, String), RpcError> {
    match auth {
        BitcoinRpcAuth::UserPass { user, pass } => {
            Ok((user.clone(), pass.expose_secret().to_string()))
        }
        BitcoinRpcAuth::CookieFile { path } => {
            let raw = std::fs::read_to_string(path).map_err(|_| RpcError::Auth)?;
            let trimmed = raw.trim();
            let (user, pass) = trimmed.split_once(':').ok_or(RpcError::Auth)?;
            Ok((user.to_string(), pass.to_string()))
        }
    }
}

#[derive(Debug, Error)]
pub enum RpcError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("timed out")]
    Timeout,
    #[error("http status {0}")]
    HttpStatus(u16),
    #[error("bitcoind returned error {code}: {message}")]
    BitcoindError { code: i32, message: String },
    #[error("decode error: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("authentication failed")]
    Auth,
}

impl RpcError {
    /// Map an RPC error to the cross-collector error taxonomy. This is
    /// the boundary translation per ADR-C3 §C3.8: every collector that
    /// fronts a bitcoind RPC surfaces the same `CollectionErrorKind`
    /// taxonomy upward, regardless of how the failure shows up at the
    /// HTTP / JSON-RPC layer.
    pub fn collection_error_kind(&self) -> CollectionErrorKind {
        match self {
            RpcError::Network(_) => CollectionErrorKind::Unreachable,
            RpcError::Timeout => CollectionErrorKind::Timeout,
            RpcError::Auth => CollectionErrorKind::AuthenticationFailed,
            RpcError::HttpStatus(_) => CollectionErrorKind::ProtocolError,
            RpcError::BitcoindError { .. } => CollectionErrorKind::InvalidResponse,
            RpcError::Decode(_) => CollectionErrorKind::DecodeError,
        }
    }
}

#[derive(Debug, Serialize)]
struct RpcRequest<'a> {
    jsonrpc: &'a str,
    id: &'a str,
    method: &'a str,
    params: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct RpcResponse<T> {
    result: Option<T>,
    error: Option<RpcErrorEnvelope>,
}

#[derive(Debug, Deserialize)]
struct RpcErrorEnvelope {
    code: i32,
    message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetBlockchainInfoResponse {
    pub chain: String,
    pub blocks: u64,
    pub headers: u64,
    #[serde(default)]
    pub bestblockhash: Option<String>,
    pub verificationprogress: f64,
    #[serde(default)]
    pub initialblockdownload: bool,
    #[serde(default)]
    pub pruned: bool,
    #[serde(default)]
    pub size_on_disk: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetMempoolInfoResponse {
    #[serde(default)]
    pub loaded: bool,
    pub size: u64,
    pub bytes: u64,
    pub usage: u64,
    pub maxmempool: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetNetworkInfoResponse {
    pub version: u64,
    pub subversion: String,
    pub protocolversion: u64,
    pub connections: u64,
    #[serde(default)]
    pub connections_in: Option<u64>,
    #[serde(default)]
    pub connections_out: Option<u64>,
    #[serde(default)]
    pub networkactive: Option<bool>,
}

pub type GetPeerInfoResponse = Vec<PeerInfoEntry>;

#[derive(Debug, Clone, Deserialize)]
pub struct PeerInfoEntry {
    #[serde(default)]
    pub inbound: bool,
    #[serde(default)]
    pub addr: Option<String>,
    #[serde(default)]
    pub subver: Option<String>,
    /// Bitcoin Core ≥ 0.21 exposes a `connection_type` string
    /// (`outbound-full-relay`, `block-relay-only`, `inbound`, etc.).
    #[serde(default)]
    pub connection_type: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::registry::BitcoinRpcAuth;
    use secrecy::SecretString;
    use std::io::Write;
    use std::net::SocketAddr;
    use std::sync::Arc;
    use tempfile::NamedTempFile;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::Notify;

    /// Tiny hand-rolled HTTP/1.1 server that reads one request, runs a
    /// caller-supplied handler against the requested method, and writes
    /// a single response. Enough to drive the JSON-RPC client without a
    /// real Bitcoin Core node.
    enum Reply {
        Json(serde_json::Value),
        Status(u16),
        Hang,
    }

    async fn spawn_mock(handler: impl Fn(&str) -> Reply + Send + Sync + 'static) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handler = Arc::new(handler);
        tokio::spawn(async move {
            loop {
                let (mut socket, _) = match listener.accept().await {
                    Ok(pair) => pair,
                    Err(_) => return,
                };
                let handler = handler.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 8192];
                    let n = socket.read(&mut buf).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    let body_start = req.find("\r\n\r\n").map(|i| i + 4).unwrap_or(req.len());
                    let body = &req[body_start..];
                    let method = serde_json::from_str::<serde_json::Value>(body)
                        .ok()
                        .and_then(|v| v.get("method").and_then(|m| m.as_str()).map(str::to_string))
                        .unwrap_or_default();
                    match handler(&method) {
                        Reply::Json(value) => {
                            let body = serde_json::to_vec(&value).unwrap();
                            let response = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                body.len()
                            );
                            socket.write_all(response.as_bytes()).await.ok();
                            socket.write_all(&body).await.ok();
                        }
                        Reply::Status(code) => {
                            let response = format!(
                                "HTTP/1.1 {} X\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                                code
                            );
                            socket.write_all(response.as_bytes()).await.ok();
                        }
                        Reply::Hang => {
                            // Hold the connection open until peer drops.
                            let never = Notify::new();
                            never.notified().await;
                        }
                    }
                });
            }
        });
        addr
    }

    fn client(addr: SocketAddr, timeout_ms: u64) -> BitcoinRpcClient {
        BitcoinRpcClient::new(
            format!("http://{}/", addr),
            BitcoinRpcAuth::UserPass {
                user: "rpcuser".into(),
                pass: SecretString::from("rpcpass".to_string()),
            },
            reqwest::Client::new(),
            Duration::from_millis(timeout_ms),
        )
    }

    fn ok_envelope(result: serde_json::Value) -> serde_json::Value {
        serde_json::json!({ "result": result, "error": null, "id": "bithound" })
    }

    #[tokio::test]
    async fn get_blockchain_info_decodes_typed_response() {
        let addr = spawn_mock(|method| {
            assert_eq!(method, "getblockchaininfo");
            Reply::Json(ok_envelope(serde_json::json!({
                "chain": "main",
                "blocks": 800_000,
                "headers": 800_005,
                "bestblockhash": "deadbeef",
                "verificationprogress": 0.9999,
                "initialblockdownload": false,
                "pruned": false,
                "size_on_disk": 500_000_000_000u64,
            })))
        })
        .await;
        let resp = client(addr, 2_000).get_blockchain_info().await.unwrap();
        assert_eq!(resp.chain, "main");
        assert_eq!(resp.blocks, 800_000);
        assert!(!resp.initialblockdownload);
    }

    #[tokio::test]
    async fn get_mempool_info_decodes_typed_response() {
        let addr = spawn_mock(|method| {
            assert_eq!(method, "getmempoolinfo");
            Reply::Json(ok_envelope(serde_json::json!({
                "loaded": true,
                "size": 12_000,
                "bytes": 8_000_000,
                "usage": 24_000_000,
                "maxmempool": 300_000_000,
            })))
        })
        .await;
        let resp = client(addr, 2_000).get_mempool_info().await.unwrap();
        assert_eq!(resp.size, 12_000);
        assert!(resp.loaded);
    }

    #[tokio::test]
    async fn get_network_info_decodes_typed_response() {
        let addr = spawn_mock(|method| {
            assert_eq!(method, "getnetworkinfo");
            Reply::Json(ok_envelope(serde_json::json!({
                "version": 250_000,
                "subversion": "/Satoshi:25.0.0/",
                "protocolversion": 70016,
                "connections": 9,
                "connections_in": 1,
                "connections_out": 8,
                "networkactive": true,
            })))
        })
        .await;
        let resp = client(addr, 2_000).get_network_info().await.unwrap();
        assert_eq!(resp.connections, 9);
        assert_eq!(resp.connections_in, Some(1));
        assert_eq!(resp.networkactive, Some(true));
    }

    #[tokio::test]
    async fn get_peer_info_decodes_typed_response() {
        let addr = spawn_mock(|method| {
            assert_eq!(method, "getpeerinfo");
            Reply::Json(ok_envelope(serde_json::json!([
                { "inbound": false, "addr": "1.2.3.4:8333", "subver": "/Satoshi:25.0.0/", "connection_type": "outbound-full-relay" },
                { "inbound": true,  "addr": "5.6.7.8:51000", "subver": "/Satoshi:24.0.0/", "connection_type": "inbound" },
            ])))
        })
        .await;
        let resp = client(addr, 2_000).get_peer_info().await.unwrap();
        assert_eq!(resp.len(), 2);
        assert!(!resp[0].inbound);
        assert!(resp[1].inbound);
    }

    #[tokio::test]
    async fn slow_server_triggers_timeout() {
        let addr = spawn_mock(|_| Reply::Hang).await;
        let err = client(addr, 100).get_blockchain_info().await.unwrap_err();
        assert!(matches!(err, RpcError::Timeout));
    }

    #[tokio::test]
    async fn http_401_maps_to_auth_error() {
        let addr = spawn_mock(|_| Reply::Status(401)).await;
        let err = client(addr, 2_000).get_blockchain_info().await.unwrap_err();
        assert!(matches!(err, RpcError::Auth));
    }

    #[tokio::test]
    async fn http_500_maps_to_http_status_error() {
        let addr = spawn_mock(|_| Reply::Status(500)).await;
        let err = client(addr, 2_000).get_blockchain_info().await.unwrap_err();
        assert!(matches!(err, RpcError::HttpStatus(500)));
    }

    #[tokio::test]
    async fn bitcoind_error_envelope_surfaces_as_bitcoind_error() {
        let addr = spawn_mock(|_| {
            Reply::Json(serde_json::json!({
                "result": null,
                "error": { "code": -5, "message": "Block not found" },
                "id": "bithound",
            }))
        })
        .await;
        let err = client(addr, 2_000).get_blockchain_info().await.unwrap_err();
        match err {
            RpcError::BitcoindError { code, message } => {
                assert_eq!(code, -5);
                assert!(message.contains("not found"));
            }
            other => panic!("expected BitcoindError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn malformed_response_maps_to_decode_error() {
        let addr = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let bound = addr.local_addr().unwrap();
        tokio::spawn(async move {
            if let Ok((mut socket, _)) = addr.accept().await {
                let mut buf = vec![0u8; 4096];
                let _ = socket.read(&mut buf).await;
                let body = b"not valid json";
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                socket.write_all(header.as_bytes()).await.ok();
                socket.write_all(body).await.ok();
            }
        });
        let err = client(bound, 2_000)
            .get_blockchain_info()
            .await
            .unwrap_err();
        assert!(matches!(err, RpcError::Decode(_)));
    }

    #[test]
    fn cookie_file_auth_parses_user_pass() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "__cookie__:abcd1234").unwrap();
        let auth = BitcoinRpcAuth::CookieFile {
            path: file.path().to_string_lossy().to_string(),
        };
        let (user, pass) = resolve_auth(&auth).expect("resolve");
        assert_eq!(user, "__cookie__");
        assert_eq!(pass, "abcd1234");
    }

    #[test]
    fn cookie_file_auth_missing_file_maps_to_auth_error() {
        let auth = BitcoinRpcAuth::CookieFile {
            path: "/nonexistent/path/.cookie".into(),
        };
        assert!(matches!(resolve_auth(&auth), Err(RpcError::Auth)));
    }

    /// Integration test against a real regtest Bitcoin Core node.
    /// Gated behind `BITHOUND_TEST_REGTEST_URL` (e.g.
    /// `http://rpcuser:rpcpass@127.0.0.1:18443`). CI skips when unset.
    #[tokio::test]
    async fn regtest_integration() {
        let Ok(url) = std::env::var("BITHOUND_TEST_REGTEST_URL") else {
            return;
        };
        let parsed = reqwest::Url::parse(&url).expect("BITHOUND_TEST_REGTEST_URL parse");
        let user = parsed.username().to_string();
        let pass = parsed.password().unwrap_or("").to_string();
        let mut bare = parsed.clone();
        bare.set_username("").ok();
        bare.set_password(None).ok();
        let client = BitcoinRpcClient::new(
            bare.to_string(),
            BitcoinRpcAuth::UserPass {
                user,
                pass: SecretString::from(pass),
            },
            reqwest::Client::new(),
            Duration::from_secs(5),
        );
        let info = client
            .get_blockchain_info()
            .await
            .expect("regtest reachable");
        assert!(info.chain == "regtest" || info.chain == "signet" || info.chain == "test");
        client.get_mempool_info().await.expect("mempool");
        client.get_network_info().await.expect("network");
        client.get_peer_info().await.expect("peers");
    }
}
