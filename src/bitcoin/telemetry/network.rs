use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    bitcoin::{client::BitcoinClient, types::NetworkMetrics},
    telemetry::{Probe, ProbeError},
};
/// Probe that collects [`NetworkMetrics`] via `getnetworkinfo`.
#[derive(Debug)]
pub struct BitcoinNetworkProbe {
    rpc: Arc<dyn BitcoinClient>,
}

impl BitcoinNetworkProbe {
    /// Build a probe sharing the given RPC client.
    pub fn new(rpc: Arc<dyn BitcoinClient>) -> Self {
        Self { rpc }
    }
}

#[async_trait]
impl Probe for BitcoinNetworkProbe {
    type Output = NetworkMetrics;

    async fn collect(&self) -> Result<Self::Output, ProbeError> {
        let info = self
            .rpc
            .get_network_info()
            .await
            .map_err(|e| ProbeError::Transport(e.to_string()))?;

        let metrics = NetworkMetrics {
            connections: info.connections,
            inbound_conns: info.connections_in,
            outbound_conns: info.connections_out,
            network_active: info.network_active,
            time_offset: info.time_offset as i64,
        };

        Ok(metrics)
    }
}
