//! System-level telemetry domain and OS-level primitives.
//!
//! Two distinct surfaces:
//!
//! - **Domain** — [`SystemTelemetry`] holds signals not tied to any specific
//!   node binary (filesystem capacity today; CPU load, NTP-daemon health,
//!   etc. in future passes). Consumed alongside per-node snapshots by
//!   cross-cutting incident detectors (e.g. X1 disk fill).
//! - **Primitives** — [`ProcessProbe`] is exposed for node domains to
//!   instantiate against their own backend process (bitcoind, lnd, …). It
//!   lives here because the implementation is OS-level and identical across
//!   domains; only the [`PidSource`] differs.

mod process;
mod telemetry;
mod types;

pub use process::{ProcessConfig, ProcessProbe};
pub use telemetry::{DiskConfig, DiskProbe, SystemProbeConfigs, SystemTelemetry};
pub use types::{DiskMetrics, PidSource, ProcessMetrics, SystemSnapshot};
