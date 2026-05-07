use std::sync::Arc;

use async_trait::async_trait;
use tokio::{sync::mpsc, time::Duration};

use crate::{
    bitcoin::{
        client::BitcoinClient,
        types::{ChainMetrics, NetworkMetrics, PeerMetrics},
    },
    telemetry::{ProbeConfig, ProbeEventStream, ProbeObservation, ProbeRunner},
};

mod chain;
mod network;
mod peers;

pub use chain::*;
pub use network::*;
pub use peers::*;

#[derive(Debug)]
pub enum BitcoinProbeEvent {
    Chain(ProbeObservation<ChainMetrics>),
    Network(ProbeObservation<NetworkMetrics>),
    Peer(ProbeObservation<PeerMetrics>),
}

#[derive(Debug, Clone)]
pub struct BitcoinProbeConfigs {
    pub chain: ProbeConfig,
    pub network: ProbeConfig,
    pub peers: ProbeConfig,
}

impl Default for BitcoinProbeConfigs {
    fn default() -> Self {
        Self {
            chain: ProbeConfig {
                interval: Duration::from_secs(5),
                timeout: Duration::from_secs(2),
            },
            network: ProbeConfig {
                interval: Duration::from_secs(10),
                timeout: Duration::from_secs(2),
            },
            peers: ProbeConfig {
                interval: Duration::from_secs(10),
                timeout: Duration::from_secs(2),
            },
        }
    }
}

pub struct BitcoinProbeEventStream {
    probes: Vec<ProbeRunner>,
    rx: mpsc::Receiver<BitcoinProbeEvent>,
}

impl BitcoinProbeEventStream {
    pub fn new(configs: BitcoinProbeConfigs, rpc: Arc<dyn BitcoinClient>) -> Self {
        let (tx, rx) = mpsc::channel::<BitcoinProbeEvent>(1024);

        let probes = vec![
            ProbeRunner::run_probe(
                BitcoinChainProbe::new(rpc.clone()),
                configs.chain,
                tx.clone(),
            ),
            ProbeRunner::run_probe(
                BitcoinNetworkProbe::new(rpc.clone()),
                configs.network,
                tx.clone(),
            ),
            ProbeRunner::run_probe(
                BitcoinPeerProbe::new(rpc.clone()),
                configs.peers,
                tx.clone(),
            ),
        ];

        Self { probes, rx }
    }
}

#[async_trait]
impl ProbeEventStream for BitcoinProbeEventStream {
    type Event = BitcoinProbeEvent;

    async fn next_event(&mut self) -> Option<Self::Event> {
        self.rx.recv().await
    }

    fn abort(self) {
        for probe in self.probes {
            probe.abort();
        }
    }
}
