//! Monitored-entity blocks. V0 ships `[[bitcoin_nodes]]`; LND and
//! generic hosts are V0.1+ but their shapes live here so the schema
//! is in one place.

use serde::Deserialize;

/// `[[bitcoin_nodes]]` array-of-tables.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BitcoinNodeConfig {
    /// Stable identifier referenced by `[[collectors]].target`.
    pub id: String,
    pub rpc_url: String,

    /// Optional ZMQ endpoint for the subscription collector (V0.1+).
    /// Parsing it now keeps the V0 schema forward-compatible.
    #[serde(default)]
    pub zmq_endpoint: Option<String>,

    pub auth: BitcoinAuthConfig,
}

/// `[bitcoin_nodes.auth]` tagged enum. `user_pass` references the
/// password via an env var name (the actual value is never written
/// in TOML); `cookie_file` reads bitcoind's own cookie so it doesn't
/// need a secret reference at all.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum BitcoinAuthConfig {
    UserPass { user: String, password_env: String },
    CookieFile { path: String },
}

/// `[[lnd_nodes]]` — wired in v0.0.8.0 via the LND gRPC polling
/// collector. `grpc_endpoint` must include the `https://` scheme;
/// `macaroon_env` names the env var holding the macaroon bytes
/// (hex-encoded at construction); `tls_cert_path` points at LND's
/// self-signed cert (the only TLS root the client trusts).
///
/// `chain_backend_target_bitcoind_id` ties this LND node to the
/// bitcoind it should be cross-correlated against for the
/// `lnd.chain_backend_lag` rule. Optional when exactly one bitcoind
/// is configured (the runtime resolves it automatically); required
/// for multi-bitcoind deployments.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LndNodeConfig {
    pub id: String,
    pub grpc_endpoint: String,
    #[serde(default)]
    pub rest_endpoint: Option<String>,
    pub macaroon_env: String,
    pub tls_cert_path: String,
    #[serde(default)]
    pub chain_backend_target_bitcoind_id: Option<String>,
}

/// `[[hosts]]` — reserved for V0.1+.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostConfig {
    pub id: String,
}
