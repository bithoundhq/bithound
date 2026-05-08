//! Per-process resource and lifecycle probe.
//!
//! Used by node domains to monitor their own backend process (e.g.
//! `BitcoinTelemetry` instantiates a `ProcessProbe` pointed at `bitcoind`).
//! Lives in `system` because the implementation is OS-level and identical
//! across domains; only the [`PidSource`] and timing differ.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use tokio::sync::Mutex;

use crate::{
    system::types::{PidSource, ProcessMetrics},
    telemetry::{Probe, ProbeConfig, ProbeError},
};

/// Configuration for a single [`ProcessProbe`] instance.
#[derive(Debug, Clone)]
pub struct ProcessConfig {
    pub probe: ProbeConfig,
    pub pid_source: PidSource,
}

/// Probe that reports resource usage for a target process via `sysinfo`.
///
/// Holds a stateful `sysinfo::System` behind a mutex so consecutive
/// collections share CPU baselines — `sysinfo`'s CPU-usage computation
/// requires two refreshes to produce a meaningful percentage. The first
/// observation may therefore report `cpu_pct == 0`; subsequent ones reflect
/// usage since the previous tick.
///
/// PID resolution happens at each collection so that pidfile-backed sources
/// pick up new PIDs when the target process restarts.
#[derive(Debug)]
pub struct ProcessProbe {
    pid_source: PidSource,
    sys: Arc<Mutex<System>>,
}

impl ProcessProbe {
    /// Build a probe targeting the process resolved by `pid_source`.
    pub fn new(pid_source: PidSource) -> Self {
        Self {
            pid_source,
            sys: Arc::new(Mutex::new(System::new())),
        }
    }

    async fn resolve_pid(&self) -> Result<Pid, ProbeError> {
        match &self.pid_source {
            PidSource::Explicit(pid) => Ok(Pid::from_u32(*pid)),
            PidSource::Pidfile(path) => {
                let raw = tokio::fs::read_to_string(path).await.map_err(|e| {
                    ProbeError::Transport(format!("read pidfile {}: {e}", path.display()))
                })?;
                let pid: u32 = raw.trim().parse().map_err(|e| {
                    ProbeError::Decode(format!("parse pid from {}: {e}", path.display()))
                })?;
                Ok(Pid::from_u32(pid))
            }
            PidSource::Either { pidfile, fallback } => {
                match tokio::fs::read_to_string(pidfile).await {
                    Ok(raw) => match raw.trim().parse::<u32>() {
                        Ok(pid) => Ok(Pid::from_u32(pid)),
                        Err(_) => Ok(Pid::from_u32(*fallback)),
                    },
                    Err(_) => Ok(Pid::from_u32(*fallback)),
                }
            }
        }
    }
}

#[async_trait]
impl Probe for ProcessProbe {
    type Output = ProcessMetrics;

    async fn collect(&self) -> Result<Self::Output, ProbeError> {
        let pid = self.resolve_pid().await?;
        let sys = self.sys.clone();

        tokio::task::spawn_blocking(move || -> Result<ProcessMetrics, ProbeError> {
            let mut sys = sys.blocking_lock();

            sys.refresh_processes_specifics(
                ProcessesToUpdate::Some(&[pid]),
                true,
                ProcessRefreshKind::everything(),
            );

            let proc = sys
                .process(pid)
                .ok_or_else(|| ProbeError::Transport(format!("process {pid} not found")))?;

            let start_time = DateTime::<Utc>::from_timestamp(proc.start_time() as i64, 0)
                .ok_or_else(|| {
                    ProbeError::Decode(format!("invalid start_time: {}", proc.start_time()))
                })?;

            Ok(ProcessMetrics {
                pid: pid.as_u32(),
                rss_bytes: proc.memory(),
                cpu_pct: proc.cpu_usage(),
                fd_count: proc.open_files().map(|n| n as u32),
                start_time,
            })
        })
        .await
        .map_err(|e| ProbeError::Transport(format!("join: {e}")))?
    }
}
