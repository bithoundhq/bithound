use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    bitcoin::{client::BitcoinClient, types::PeerMetrics},
    telemetry::{Probe, ProbeError},
};

/// Probe that collects [`PeerMetrics`] via `getpeerinfo`.
///
/// Reduces the per-peer response into min / avg / max ping and the worst-case
/// `synced_headers` / `synced_blocks` across the peer set, so a single lagging
/// peer surfaces as a low minimum.
#[derive(Debug)]
pub struct BitcoinPeerProbe {
    rpc: Arc<dyn BitcoinClient>,
}

impl BitcoinPeerProbe {
    /// Build a probe sharing the given RPC client.
    pub fn new(rpc: Arc<dyn BitcoinClient>) -> Self {
        Self { rpc }
    }
}

#[async_trait]
impl Probe for BitcoinPeerProbe {
    type Output = PeerMetrics;

    async fn collect(&self) -> Result<Self::Output, ProbeError> {
        let info = self
            .rpc
            .get_peer_info()
            .await
            .map_err(|e| ProbeError::Transport(e.to_string()))?;

        let avg_ping_ms = if info.0.is_empty() {
            0.0
        } else {
            info.0.iter().filter_map(|p| p.ping_time).sum::<f64>() / info.0.len() as f64
        };

        let min_ping_ms = info
            .0
            .iter()
            .filter_map(|p| p.ping_time)
            .reduce(f64::min)
            .unwrap_or(0.0);
        let max_ping_ms = info
            .0
            .iter()
            .filter_map(|p| p.ping_time)
            .reduce(f64::max)
            .unwrap_or(0.0);

        let synced_headers_min = info
            .0
            .iter()
            .filter_map(|p| p.synced_headers)
            .min()
            .unwrap_or(0);
        let synced_blocks_min = info
            .0
            .iter()
            .filter_map(|p| p.synced_blocks)
            .min()
            .unwrap_or(0);

        let metrics = PeerMetrics {
            min_ping_ms,
            max_ping_ms,
            avg_ping_ms,
            synced_headers_min,
            synced_blocks_min,
        };

        Ok(metrics)
    }
}
