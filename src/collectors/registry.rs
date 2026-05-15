//! Resolves `CollectorTarget` identifiers to concrete connection details.
//! Loaded once from config at sidecar startup; collectors look up their target here.
use std::collections::HashMap;

use secrecy::SecretString;

use crate::shared::types::{BitcoinNodeId, HostId, LndNodeId};

#[derive(Debug, Clone, Default)]
pub struct NodeRegistry {
    pub bitcoin_nodes: HashMap<BitcoinNodeId, BitcoinNodeConnection>,
    pub lnd_nodes: HashMap<LndNodeId, LndNodeConnection>,
    pub hosts: HashMap<HostId, HostConnection>,
}

#[derive(Debug, Clone)]
pub struct BitcoinNodeConnection {
    pub rpc_url: String,
    pub rpc_auth: BitcoinRpcAuth,
    pub zmq_endpoint: Option<String>,
}

#[derive(Debug, Clone)]
pub enum BitcoinRpcAuth {
    UserPass { user: String, pass: SecretString },
    CookieFile { path: String },
}

#[derive(Debug, Clone)]
pub struct LndNodeConnection {
    pub grpc_endpoint: String,
    pub rest_endpoint: Option<String>,
    pub macaroon: SecretString,
    pub tls_cert_path: String,
}

#[derive(Debug, Clone, Default)]
pub struct HostConnection {
    // Local target — no remote endpoint. Reserved for future fields:
    //   - alternative /proc root for testing
    //   - per-host filters or labels
}
