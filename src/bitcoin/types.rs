use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct ChainMetrics {
    pub blocks: i64,
    pub headers: i64,
    pub verification_progress: f64,
    pub initial_block_download: bool,
    pub pruned: bool,
}

impl ChainMetrics {
    pub fn tip_lag(&self) -> i64 {
        self.headers - self.blocks
    }
}

#[derive(Debug, Clone)]
pub struct NetworkMetrics {
    pub connections: usize,
    pub inbound_conns: usize,
    pub outbound_conns: usize,
    pub network_active: bool,
}

#[derive(Debug, Clone)]
pub struct PeerMetrics {
    pub min_ping_ms: f64,
    pub avg_ping_ms: f64,
    pub max_ping_ms: f64,
    pub synced_headers_min: i64,
    pub synced_blocks_min: i64,
}
