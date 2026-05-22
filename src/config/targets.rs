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

/// `[[lnd_nodes]]` — reserved for V0.1+. Defined now so the TOML
/// schema is closed off (`deny_unknown_fields` on the parent table).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LndNodeConfig {
    pub id: String,
    pub grpc_endpoint: String,
    #[serde(default)]
    pub rest_endpoint: Option<String>,
    pub macaroon_env: String,
    pub tls_cert_path: String,
}

/// `[[hosts]]` — reserved for V0.1+.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostConfig {
    pub id: String,
}
