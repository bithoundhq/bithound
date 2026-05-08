use std::path::PathBuf;

use async_trait::async_trait;

use crate::{
    system::types::DiskMetrics,
    telemetry::{Probe, ProbeError},
};

/// Probe that reports filesystem capacity for the volume containing a given
/// path, via `sysinfo::Disks`.
///
/// On every collection the probe refreshes the mounted-disk list and finds
/// the entry whose `mount_point` is the longest prefix of the configured
/// path. The path is canonicalized once per collection so symlinks and
/// relative components are resolved before matching.
#[derive(Debug)]
pub struct DiskProbe {
    path: PathBuf,
}

impl DiskProbe {
    /// Build a probe targeting the filesystem containing `path`.
    ///
    /// `path` may be a directory or a regular file — only its containing
    /// filesystem is measured. The path does not need to exist at probe
    /// construction time, but must resolve at collection time or the probe
    /// will surface `Failed`.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[async_trait]
impl Probe for DiskProbe {
    type Output = DiskMetrics;

    async fn collect(&self) -> Result<Self::Output, ProbeError> {
        let path = self.path.clone();

        tokio::task::spawn_blocking(move || -> Result<DiskMetrics, ProbeError> {
            let canonical = path
                .canonicalize()
                .map_err(|e| ProbeError::Transport(format!("canonicalize: {e}")))?;

            let disks = sysinfo::Disks::new_with_refreshed_list();
            let disk = disks
                .iter()
                .filter(|d| canonical.starts_with(d.mount_point()))
                .max_by_key(|d| d.mount_point().as_os_str().len())
                .ok_or_else(|| {
                    ProbeError::Transport(format!(
                        "no mounted filesystem for {}",
                        canonical.display()
                    ))
                })?;

            Ok(DiskMetrics {
                path: canonical,
                total_bytes: disk.total_space(),
                available_bytes: disk.available_space(),
            })
        })
        .await
        .map_err(|e| ProbeError::Transport(format!("join: {e}")))?
    }
}
