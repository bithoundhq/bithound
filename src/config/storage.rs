use std::path::PathBuf;

use serde::Deserialize;

/// Top-level `[storage]` block. SQLite-only in V0.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    /// SQLite database file. Parent directory must be writable;
    /// `Config::load_from_args_and_env` will try to create it if
    /// missing and surface a clear error if it can't.
    pub db_path: PathBuf,

    #[serde(default)]
    pub retention: RetentionConfig,
}

/// `[storage.retention]` block. All fields are optional; defaults
/// are set from `Default for RetentionConfig` below.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionConfig {
    #[serde(default = "default_obs_max_age")]
    pub observations_max_age_days: u32,

    #[serde(default = "default_incidents_max_age")]
    pub incidents_max_age_days: u32,

    #[serde(default = "default_suppressions_grace")]
    pub suppressions_grace_days: u32,

    #[serde(default = "default_vacuum_interval")]
    pub vacuum_interval_hours: u32,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            observations_max_age_days: default_obs_max_age(),
            incidents_max_age_days: default_incidents_max_age(),
            suppressions_grace_days: default_suppressions_grace(),
            vacuum_interval_hours: default_vacuum_interval(),
        }
    }
}

fn default_obs_max_age() -> u32 {
    30
}
fn default_incidents_max_age() -> u32 {
    365
}
fn default_suppressions_grace() -> u32 {
    90
}
fn default_vacuum_interval() -> u32 {
    24
}
