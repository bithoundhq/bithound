use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    bitcoin::{client::BitcoinClient, telemetry::BitcoinProbeEvent, types::ChainMetrics},
    telemetry::{Probe, ProbeError, ProbeObservation},
};

#[derive(Debug)]
pub struct BitcoinChainProbe {
    rpc: Arc<dyn BitcoinClient>,
}

impl BitcoinChainProbe {
    pub fn new(rpc: Arc<dyn BitcoinClient>) -> Self {
        Self { rpc }
    }
}

#[async_trait]
impl Probe for BitcoinChainProbe {
    type Output = ChainMetrics;

    async fn observe(&self) -> Result<Self::Output, ProbeError> {
        let info = self.rpc.get_blockchain_info().await?;

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

impl From<ProbeObservation<ChainMetrics>> for BitcoinProbeEvent {
    fn from(value: ProbeObservation<ChainMetrics>) -> Self {
        Self::Chain(value)
    }
}
