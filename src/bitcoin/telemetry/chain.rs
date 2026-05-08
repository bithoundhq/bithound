use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    bitcoin::{client::BitcoinClient, types::ChainMetrics},
    telemetry::{Probe, ProbeError},
};

/// Probe that collects [`ChainMetrics`] via `getblockchaininfo`.
#[derive(Debug)]
pub struct BitcoinChainProbe {
    rpc: Arc<dyn BitcoinClient>,
}

impl BitcoinChainProbe {
    /// Build a probe sharing the given RPC client.
    pub fn new(rpc: Arc<dyn BitcoinClient>) -> Self {
        Self { rpc }
    }
}

#[async_trait]
impl Probe for BitcoinChainProbe {
    type Output = ChainMetrics;

    async fn collect(&self) -> Result<Self::Output, ProbeError> {
        let info = self
            .rpc
            .get_blockchain_info()
            .await
            .map_err(|e| ProbeError::Transport(e.to_string()))?;

        let chain_metrics = ChainMetrics {
            blocks: info.blocks,
            headers: info.headers,
            verification_progress: info.verification_progress,
            initial_block_download: info.initial_block_download,
            pruned: info.pruned,
        };

        Ok(chain_metrics)
    }
}
