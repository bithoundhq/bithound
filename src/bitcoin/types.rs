//! Public Bitcoin telemetry value types.
//!
//! Each `*Metrics` struct is the success payload of one probe. [`BitcoinSnapshot`]
//! is the aggregate consumer-facing view — one [`ProbeSnapshot`] per probe, with
//! TTL already evaluated at projection time.

use crate::telemetry::ProbeSnapshot;

/// Chain-tip and sync state, derived from `getblockchaininfo`.
#[derive(Debug, Clone)]
pub struct ChainMetrics {
    pub blocks: i64,
    pub headers: i64,
    pub verification_progress: f64,
    pub initial_block_download: bool,
    pub pruned: bool,
}

impl ChainMetrics {
    /// Headers known beyond the current validated tip.
    ///
    /// Zero indicates the node is fully synced; positive values quantify the
    /// validation backlog (headers seen but blocks not yet processed).
    pub fn tip_lag(&self) -> i64 {
        self.headers - self.blocks
    }
}

/// Connectivity state, derived from `getnetworkinfo`.
///
/// `time_offset` is bitcoind's median peer-derived clock offset (seconds);
/// per the incident catalog (X2), magnitudes above ~30s indicate either local
/// clock skew or that we're listening to adversarial peers. This is the
/// peer-derived view of clock skew — for the independent system-NTP view, see
/// the system telemetry domain.
#[derive(Debug, Clone)]
pub struct NetworkMetrics {
    pub connections: usize,
    pub inbound_conns: usize,
    pub outbound_conns: usize,
    pub network_active: bool,
    pub time_offset: i64,
}

/// Aggregated peer-set statistics, derived from `getpeerinfo`.
///
/// Per-peer pings are reduced to min / avg / max across the live peer set,
/// and `synced_*_min` reports the worst-synced peer — so a single lagging
/// peer is visible as a low minimum without dragging the average.
#[derive(Debug, Clone)]
pub struct PeerMetrics {
    pub min_ping_ms: f64,
    pub avg_ping_ms: f64,
    pub max_ping_ms: f64,
    pub synced_headers_min: i64,
    pub synced_blocks_min: i64,
}

/// Mempool size and fee thresholds, derived from `getmempoolinfo`.
///
/// `size` and `bytes` describe the current contents (transaction count and
/// total serialized size); `usage` is the in-memory footprint in bytes. The
/// three `*_fee` fields are the dynamic and static minimum fee rates the node
/// will accept, expressed in BTC/kvB.
#[derive(Debug, Clone)]
pub struct MempoolMetrics {
    pub size: i64,
    pub bytes: i64,
    pub usage: i64,
    pub mempool_min_fee: f64,
    pub min_relay_tx_fee: f64,
    pub incremental_relay_fee: f64,
}

/// Aggregate Bitcoin telemetry view at a single point in time.
///
/// This is the public consumer-facing snapshot: one [`ProbeSnapshot`] per
/// probe, with TTL already applied. Cross-probe invariants (synced, degraded,
/// healthy, etc.) are intended to live as methods on this type — they read
/// multiple fields without needing access to any internal probe state.
#[derive(Debug, Clone)]
pub struct BitcoinSnapshot {
    pub chain: ProbeSnapshot<ChainMetrics>,
    pub network: ProbeSnapshot<NetworkMetrics>,
    pub peers: ProbeSnapshot<PeerMetrics>,
    pub mempool: ProbeSnapshot<MempoolMetrics>,
}
