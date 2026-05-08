//! System telemetry pipeline.
//!
//! Wires the system-level probes into [`SystemTelemetry`], mirroring the
//! per-domain shape used by `bitcoin::telemetry`. For now the only probe is
//! [`DiskProbe`]; future additions (clock skew via NTP, CPU load) plug in by
//! extending [`SystemProbeConfigs`] and the aggregator's `select`.

use std::{path::PathBuf, sync::Arc};

use chrono::{Duration as ChronoDuration, Utc};
use tokio::{sync::watch, task::JoinSet, time::Duration};

use crate::{
    system::types::{DiskMetrics, SystemSnapshot},
    telemetry::{ProbeConfig, ProbeSnapshot, evaluate_ttl, spawn_probe},
};

mod disk;

pub use disk::*;

/// Configuration for a single [`DiskProbe`] instance.
///
/// `path` is forwarded to [`DiskProbe::new`]; `probe` controls the timing
/// and TTL of the spawned loop.
#[derive(Debug, Clone)]
pub struct DiskConfig {
    pub probe: ProbeConfig,
    pub path: PathBuf,
}

/// Per-probe configuration for the system telemetry domain.
///
/// Unlike [`crate::bitcoin::BitcoinProbeConfigs`], there is no [`Default`]:
/// the disk path is operator-specific (typically a node's data directory),
/// so the caller must always supply it.
#[derive(Debug, Clone)]
pub struct SystemProbeConfigs {
    pub disk: DiskConfig,
}

impl SystemProbeConfigs {
    /// Build a configuration with reasonable cadences targeting `disk_path`.
    ///
    /// The disk probe ticks every 60s with a 5s timeout and a 180s TTL —
    /// disk usage moves slowly relative to chain state, so a longer cadence
    /// keeps overhead low.
    pub fn for_disk(disk_path: PathBuf) -> Self {
        Self {
            disk: DiskConfig {
                probe: ProbeConfig {
                    interval: Duration::from_secs(60),
                    timeout: Duration::from_secs(5),
                    ttl: ChronoDuration::seconds(180),
                },
                path: disk_path,
            },
        }
    }
}

/// System domain telemetry runtime.
///
/// Owns the spawned probe tasks and an aggregator task that republishes a
/// fresh [`SystemSnapshot`] (TTL-evaluated) on every underlying probe change.
/// Same access patterns as [`crate::bitcoin::BitcoinTelemetry`]: pull-style
/// via [`Self::snapshot`], push-style via [`Self::watch`] (aggregate) or
/// [`Self::disk`] (per-probe).
#[derive(Debug)]
pub struct SystemTelemetry {
    disk: watch::Receiver<ProbeSnapshot<DiskMetrics>>,
    snapshot: watch::Receiver<Arc<SystemSnapshot>>,
    cfg: SystemProbeConfigs,
    tasks: JoinSet<()>,
}

impl SystemTelemetry {
    /// Spawn every system-domain probe plus the aggregator task.
    pub fn spawn(cfg: SystemProbeConfigs) -> Self {
        let mut tasks = JoinSet::new();

        let disk = spawn_probe(
            DiskProbe::new(cfg.disk.path.clone()),
            cfg.disk.probe.clone(),
            &mut tasks,
        );

        let initial = Self::project(&disk, &cfg);
        let (snap_tx, snap_rx) = watch::channel(Arc::new(initial));

        let mut disk_rx = disk.clone();
        let cfg_aggregator = cfg.clone();
        tasks.spawn(async move {
            loop {
                if disk_rx.changed().await.is_err() {
                    break;
                }

                let snap = Self::project(&disk_rx, &cfg_aggregator);
                if snap_tx.send(Arc::new(snap)).is_err() {
                    break;
                }
            }
        });

        Self {
            disk,
            snapshot: snap_rx,
            cfg,
            tasks,
        }
    }

    /// Build a fresh aggregate snapshot from the current per-probe state,
    /// applying TTL against the read-time clock.
    fn project(
        disk: &watch::Receiver<ProbeSnapshot<DiskMetrics>>,
        cfg: &SystemProbeConfigs,
    ) -> SystemSnapshot {
        let now = Utc::now();
        SystemSnapshot {
            disk: evaluate_ttl(disk.borrow().clone(), cfg.disk.probe.ttl, now),
        }
    }

    /// Synchronously project the latest aggregate snapshot.
    pub fn snapshot(&self) -> SystemSnapshot {
        Self::project(&self.disk, &self.cfg)
    }

    /// Subscribe to the aggregate snapshot, refreshed on any probe change.
    pub fn watch(&self) -> watch::Receiver<Arc<SystemSnapshot>> {
        self.snapshot.clone()
    }

    /// Subscribe to disk probe observations only.
    pub fn disk(&self) -> watch::Receiver<ProbeSnapshot<DiskMetrics>> {
        self.disk.clone()
    }

    /// Abort every spawned task and await their termination.
    pub async fn shutdown(mut self) {
        self.tasks.shutdown().await;
    }
}
