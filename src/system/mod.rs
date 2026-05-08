//! System-level telemetry domain.
//!
//! Holds OS-level signals that aren't tied to any specific node binary —
//! filesystem capacity today, system clock skew and CPU load in future
//! passes. Consumed alongside per-node snapshots (Bitcoin, Lightning,
//! Elements) by cross-cutting incident detectors (e.g. X1 disk fill, X2
//! clock skew).

mod telemetry;
mod types;

pub use telemetry::{DiskConfig, DiskProbe, SystemProbeConfigs, SystemTelemetry};
pub use types::{DiskMetrics, SystemSnapshot};
