use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    bitcoin::{client::BitcoinClient, telemetry::BitcoinProbeEvent, types::NetworkMetrics},
    telemetry::{Probe, ProbeError, ProbeObservation},
};
#[derive(Debug)]
pub struct BitcoinNetworkProbe {
    rpc: Arc<dyn BitcoinClient>,
}

impl BitcoinNetworkProbe {
    pub fn new(rpc: Arc<dyn BitcoinClient>) -> Self {
        Self { rpc }
    }
}

#[async_trait]
impl Probe for BitcoinNetworkProbe {
    type Output = NetworkMetrics;

    async fn observe(&self) -> Result<Self::Output, ProbeError> {
        let info = self.rpc.get_network_info().await?;

        let metrics = NetworkMetrics {
            connections: info.connections,
            inbound_conns: info.connections_in,
            outbound_conns: info.connections_out,
            network_active: info.network_active,
        };

        Ok(metrics)
    }
}

impl From<ProbeObservation<NetworkMetrics>> for BitcoinProbeEvent {
    fn from(value: ProbeObservation<NetworkMetrics>) -> Self {
        Self::Network(value)
    }
}
