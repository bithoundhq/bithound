//! Bitcoin telemetry pipeline.
//!
//! Wires the per-probe collection loops (`chain`, `network`, `peers`, `mempool`)
//! into [`BitcoinTelemetry`], which exposes both per-probe `watch` receivers
//! and a single aggregate `watch::Receiver<Arc<BitcoinSnapshot>>` that fires on
//! any underlying change.

use std::sync::Arc;

use chrono::{Duration as ChronoDuration, Utc};
use tokio::{sync::watch, task::JoinSet, time::Duration};

use crate::{
    bitcoin::{
        client::BitcoinClient,
        types::{BitcoinSnapshot, ChainMetrics, MempoolMetrics, NetworkMetrics, PeerMetrics},
    },
    telemetry::{ProbeConfig, ProbeSnapshot, evaluate_ttl, spawn_probe},
};

mod chain;
mod mempool;
mod network;
mod peers;

pub use chain::*;
pub use mempool::*;
pub use network::*;
pub use peers::*;

/// Per-probe configuration for the Bitcoin telemetry domain.
///
/// Each field maps to one probe loop. [`Default`] picks reasonable cadences
/// for a healthy node — short interval for the chain tip and mempool (which
/// move on every block / transaction), longer for network and peer state
/// (which change less often).
#[derive(Debug, Clone)]
pub struct BitcoinProbeConfigs {
    pub chain: ProbeConfig,
    pub network: ProbeConfig,
    pub peers: ProbeConfig,
    pub mempool: ProbeConfig,
}

impl Default for BitcoinProbeConfigs {
    fn default() -> Self {
        Self {
            chain: ProbeConfig {
                interval: Duration::from_secs(5),
                timeout: Duration::from_secs(2),
                ttl: ChronoDuration::seconds(15),
            },
            network: ProbeConfig {
                interval: Duration::from_secs(10),
                timeout: Duration::from_secs(2),
                ttl: ChronoDuration::seconds(30),
            },
            peers: ProbeConfig {
                interval: Duration::from_secs(10),
                timeout: Duration::from_secs(2),
                ttl: ChronoDuration::seconds(30),
            },
            mempool: ProbeConfig {
                interval: Duration::from_secs(5),
                timeout: Duration::from_secs(2),
                ttl: ChronoDuration::seconds(15),
            },
        }
    }
}

/// Bitcoin domain telemetry runtime.
///
/// Owns the spawned probe tasks and an aggregator task that republishes a
/// fresh [`BitcoinSnapshot`] (TTL-evaluated) every time any underlying probe
/// emits an observation.
///
/// Consumers can subscribe push-style via [`Self::watch`] (aggregate) or one
/// of the per-probe accessors, or pull-style via [`Self::snapshot`]. Drop the
/// runtime — or call [`Self::shutdown`] — to abort all spawned tasks; the
/// per-probe senders close as a result, and any external consumer holding a
/// receiver will see `RecvError` from `.changed()`.
#[derive(Debug)]
pub struct BitcoinTelemetry {
    chain: watch::Receiver<ProbeSnapshot<ChainMetrics>>,
    network: watch::Receiver<ProbeSnapshot<NetworkMetrics>>,
    peers: watch::Receiver<ProbeSnapshot<PeerMetrics>>,
    mempool: watch::Receiver<ProbeSnapshot<MempoolMetrics>>,
    snapshot: watch::Receiver<Arc<BitcoinSnapshot>>,
    cfg: BitcoinProbeConfigs,
    tasks: JoinSet<()>,
}

impl BitcoinTelemetry {
    /// Spawn every probe in the Bitcoin domain plus the aggregator task.
    ///
    /// The same `rpc` handle is shared across all probes via `Arc`. The
    /// returned handle owns the spawned tasks via a [`JoinSet`]; dropping it
    /// cancels in-flight collections.
    pub fn spawn(rpc: Arc<dyn BitcoinClient>, cfg: BitcoinProbeConfigs) -> Self {
        let mut tasks = JoinSet::new();

        let chain = spawn_probe(
            BitcoinChainProbe::new(rpc.clone()),
            cfg.chain.clone(),
            &mut tasks,
        );
        let network = spawn_probe(
            BitcoinNetworkProbe::new(rpc.clone()),
            cfg.network.clone(),
            &mut tasks,
        );
        let peers = spawn_probe(
            BitcoinPeerProbe::new(rpc.clone()),
            cfg.peers.clone(),
            &mut tasks,
        );
        let mempool = spawn_probe(
            BitcoinMempoolProbe::new(rpc.clone()),
            cfg.mempool.clone(),
            &mut tasks,
        );

        let initial = Self::project(&chain, &network, &peers, &mempool, &cfg);
        let (snap_tx, snap_rx) = watch::channel(Arc::new(initial));

        let mut chain_rx = chain.clone();
        let mut network_rx = network.clone();
        let mut peers_rx = peers.clone();
        let mut mempool_rx = mempool.clone();
        let cfg_aggregator = cfg.clone();
        tasks.spawn(async move {
            loop {
                tokio::select! {
                    res = chain_rx.changed() => if res.is_err() { break; },
                    res = network_rx.changed() => if res.is_err() { break; },
                    res = peers_rx.changed() => if res.is_err() { break; },
                    res = mempool_rx.changed() => if res.is_err() { break; },
                }

                let snap = Self::project(
                    &chain_rx,
                    &network_rx,
                    &peers_rx,
                    &mempool_rx,
                    &cfg_aggregator,
                );
                if snap_tx.send(Arc::new(snap)).is_err() {
                    break;
                }
            }
        });

        Self {
            chain,
            network,
            peers,
            mempool,
            snapshot: snap_rx,
            cfg,
            tasks,
        }
    }

    /// Build a fresh aggregate snapshot from the current per-probe state.
    ///
    /// Reads each `watch` receiver's current value and applies TTL against
    /// the read-time clock, so a previously-fresh `Success` may surface as
    /// `Stale` if no new observation has arrived within its TTL.
    fn project(
        chain: &watch::Receiver<ProbeSnapshot<ChainMetrics>>,
        network: &watch::Receiver<ProbeSnapshot<NetworkMetrics>>,
        peers: &watch::Receiver<ProbeSnapshot<PeerMetrics>>,
        mempool: &watch::Receiver<ProbeSnapshot<MempoolMetrics>>,
        cfg: &BitcoinProbeConfigs,
    ) -> BitcoinSnapshot {
        let now = Utc::now();
        BitcoinSnapshot {
            chain: evaluate_ttl(chain.borrow().clone(), cfg.chain.ttl, now),
            network: evaluate_ttl(network.borrow().clone(), cfg.network.ttl, now),
            peers: evaluate_ttl(peers.borrow().clone(), cfg.peers.ttl, now),
            mempool: evaluate_ttl(mempool.borrow().clone(), cfg.mempool.ttl, now),
        }
    }

    /// Synchronously project the latest aggregate snapshot.
    ///
    /// Cheap pull-style access — suitable for one-shot consumers (HTTP
    /// handlers, CLI dumps). For long-running consumers that should wake on
    /// changes, prefer [`Self::watch`] or a per-probe receiver.
    pub fn snapshot(&self) -> BitcoinSnapshot {
        Self::project(
            &self.chain,
            &self.network,
            &self.peers,
            &self.mempool,
            &self.cfg,
        )
    }

    /// Subscribe to the aggregate snapshot, refreshed on any probe change.
    ///
    /// The returned receiver holds an `Arc<BitcoinSnapshot>`; cloning is
    /// cheap and decoupled from the channel's internal storage. This is the
    /// substrate cross-probe invariants compute against — methods on
    /// [`BitcoinSnapshot`] (e.g. `is_synced`, `is_degraded`) see a single
    /// consistent view of every probe.
    pub fn watch(&self) -> watch::Receiver<Arc<BitcoinSnapshot>> {
        self.snapshot.clone()
    }

    /// Subscribe to chain probe observations only.
    pub fn chain(&self) -> watch::Receiver<ProbeSnapshot<ChainMetrics>> {
        self.chain.clone()
    }

    /// Subscribe to network probe observations only.
    pub fn network(&self) -> watch::Receiver<ProbeSnapshot<NetworkMetrics>> {
        self.network.clone()
    }

    /// Subscribe to peer probe observations only.
    pub fn peers(&self) -> watch::Receiver<ProbeSnapshot<PeerMetrics>> {
        self.peers.clone()
    }

    /// Subscribe to mempool probe observations only.
    pub fn mempool(&self) -> watch::Receiver<ProbeSnapshot<MempoolMetrics>> {
        self.mempool.clone()
    }

    /// Abort every spawned task and await their termination.
    ///
    /// Equivalent to dropping the runtime, but blocks until tasks have
    /// actually stopped — useful at process shutdown when callers want to
    /// be sure no probes are mid-call.
    pub async fn shutdown(mut self) {
        self.tasks.shutdown().await;
    }
}
