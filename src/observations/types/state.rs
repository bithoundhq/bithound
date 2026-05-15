//! Types for state observations.
use serde::{Deserialize, Serialize};

use crate::observations::types::NumericValue;

/// Structured snapshots of a subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateObservation {
    BitcoinBlockchain(BitcoinBlockchainState),
    BitcoinMempool(BitcoinMempoolState),
    BitcoinNetwork(BitcoinNetworkState),
    BitcoinPeerSummary(BitcoinPeerSummaryState),

    LndNode(LndNodeState),
    LndWallet(LndWalletState),
    LndChannelSummary(LndChannelSummaryState),

    Host(HostState),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StateName(pub String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StateValue {
    Bool(bool),
    String(String),
    Number(NumericValue),
    Object(serde_json::Value),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitcoinBlockchainState {
    pub chain: String,
    pub blocks: u64,
    pub headers: u64,
    pub best_block_hash: Option<String>,
    pub verification_progress: f64,
    pub initial_block_download: bool,
    pub pruned: bool,
    pub size_on_disk_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitcoinMempoolState {
    pub loaded: bool,
    pub tx_count: u64,
    pub bytes: u64,
    pub usage_bytes: u64,
    pub max_mempool_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitcoinNetworkState {
    pub version: u64,
    pub subversion: String,
    pub protocol_version: u64,
    pub connections: u64,
    pub connections_in: Option<u64>,
    pub connections_out: Option<u64>,
    pub network_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitcoinPeerSummaryState {
    pub peer_count: u64,
    pub inbound_count: Option<u64>,
    pub outbound_count: Option<u64>,
    pub block_relay_only_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LndNodeState {
    pub identity_pubkey: String,
    pub alias: Option<String>,
    pub version: Option<String>,
    pub num_active_channels: u64,
    pub num_inactive_channels: Option<u64>,
    pub num_pending_channels: u64,
    pub num_peers: u64,
    pub block_height: u64,
    pub synced_to_chain: bool,
    pub synced_to_graph: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LndWalletState {
    pub total_balance_sat: u64,
    pub confirmed_balance_sat: u64,
    pub unconfirmed_balance_sat: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LndChannelSummaryState {
    pub active_channels: u64,
    pub inactive_channels: u64,
    pub pending_channels: u64,
    pub total_capacity_sat: Option<u64>,
    pub local_balance_sat: u64,
    pub remote_balance_sat: u64,
    pub unsettled_balance_sat: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostState {
    pub hostname: Option<String>,
    pub os: Option<String>,
    pub kernel: Option<String>,
    pub uptime_seconds: Option<u64>,
    pub cpu_count: Option<u64>,
    pub memory_total_bytes: Option<u64>,
    pub disk_total_bytes: Option<u64>,
    pub disk_available_bytes: Option<u64>,
}
