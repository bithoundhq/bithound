//! Public system telemetry value types.

use std::path::PathBuf;

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

/// Aggregate system telemetry view at a single point in time.
///
/// The system domain holds OS-level signals that aren't tied to any specific
/// node binary — disk space, system clock skew (future), CPU load (future).
/// Cross-cutting incident detectors (e.g. X1 disk-fill projection) consume
/// this alongside per-domain snapshots.
#[derive(Debug, Clone)]
pub struct SystemSnapshot {
    pub disk: ProbeSnapshot<DiskMetrics>,
}
