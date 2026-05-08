use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    bitcoin::{client::BitcoinClient, types::MempoolMetrics},
    telemetry::{Probe, ProbeError},
};

/// Probe that collects [`MempoolMetrics`] via `getmempoolinfo`.
#[derive(Debug)]
pub struct BitcoinMempoolProbe {
    rpc: Arc<dyn BitcoinClient>,
}

impl BitcoinMempoolProbe {
    /// Build a probe sharing the given RPC client.
    pub fn new(rpc: Arc<dyn BitcoinClient>) -> Self {
        Self { rpc }
    }
}

#[async_trait]
impl Probe for BitcoinMempoolProbe {
    type Output = MempoolMetrics;

    async fn collect(&self) -> Result<Self::Output, ProbeError> {
        let info = self
            .rpc
            .get_mempool_info()
            .await
            .map_err(|e| ProbeError::Transport(e.to_string()))?;

        let metrics = MempoolMetrics {
            size: info.size,
            bytes: info.bytes,
            usage: info.usage,
            mempool_min_fee: info.mempool_min_fee,
            min_relay_tx_fee: info.min_relay_tx_fee,
            incremental_relay_fee: info.incremental_relay_fee,
        };

        Ok(metrics)
    }
}
