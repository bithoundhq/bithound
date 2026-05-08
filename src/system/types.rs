//! Public system telemetry value types.

use std::path::PathBuf;

use chrono::{DateTime, Utc};

use crate::telemetry::ProbeSnapshot;

/// Filesystem capacity stats for a single mount point, derived from `statvfs`.
///
/// `path` is the canonicalized location whose containing filesystem was
/// measured — useful for distinguishing between disks when more than one
/// `DiskProbe` is configured. `available_bytes` is the space usable by
/// non-privileged processes (i.e. excluding reserved blocks); `total_bytes`
/// is the filesystem capacity.
#[derive(Debug, Clone)]
pub struct DiskMetrics {
    pub path: PathBuf,
    pub total_bytes: u64,
    pub available_bytes: u64,
}

impl DiskMetrics {
    /// Bytes used on the measured filesystem.
    ///
    /// Derived from `total_bytes - available_bytes`, saturating at zero in
    /// the (unlikely) case that reserved-block accounting reports `available`
    /// greater than `total`.
    pub fn used_bytes(&self) -> u64 {
        self.total_bytes.saturating_sub(self.available_bytes)
    }
}

/// How a [`ProcessProbe`](crate::system::ProcessProbe) should resolve its
/// target PID at each collection.
///
/// Pidfile-driven sources are re-read on every tick so the probe survives
/// process restarts that change the PID (e.g. bitcoind crashes and is brought
/// back up by a service manager).
#[derive(Debug, Clone)]
pub enum PidSource {
    /// Read PID from the file at every collection. Fails the probe if the
    /// file is missing or unparseable.
    Pidfile(PathBuf),
    /// Use this PID directly. Probe fails when the process exits.
    Explicit(u32),
    /// Try the pidfile first; fall back to the explicit PID if the pidfile
    /// is missing or unparseable. Useful when the pidfile is normally
    /// present but the operator wants resilience against pidfile churn
    /// during service restarts.
    Either { pidfile: PathBuf, fallback: u32 },
}

/// Per-process resource and lifecycle stats.
///
/// `cpu_pct` is the percentage across all cores combined (per `sysinfo`
/// convention) — a process saturating two cores reports ~200%. The first
/// observation after probe startup may report 0 because CPU usage requires
/// two refresh samples to compute; subsequent samples are accurate.
///
/// `fd_count` is `None` on platforms where `sysinfo` cannot enumerate open
/// file descriptors (e.g. minimal Windows builds), distinguishing
/// "unmeasured" from "zero descriptors."
#[derive(Debug, Clone)]
pub struct ProcessMetrics {
    pub pid: u32,
    pub rss_bytes: u64,
    pub cpu_pct: f32,
    pub fd_count: Option<u32>,
    pub start_time: DateTime<Utc>,
}

/// Aggregate system telemetry view at a single point in time.
///
/// The system domain holds OS-level signals that aren't tied to any specific
/// node binary — disk space today, CPU load and similar in future passes.
/// Per-process metrics live inside each node domain (e.g. bitcoind's
/// `ProcessMetrics` inside `BitcoinSnapshot`) because a process is 1:1 with
/// a node, while a filesystem isn't.
#[derive(Debug, Clone)]
pub struct SystemSnapshot {
    pub disk: ProbeSnapshot<DiskMetrics>,
}
