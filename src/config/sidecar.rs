use std::path::PathBuf;

use serde::Deserialize;

/// Top-level `[sidecar]` block. Holds identity + log-filter settings.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SidecarConfig {
    /// File where the persistent `SidecarId` UUIDv7 lives. Generated
    /// on first run and reused on every subsequent start so
    /// observation provenance stays stable across restarts.
    pub id_file: PathBuf,

    /// Tracing filter string passed to `tracing_subscriber::EnvFilter`
    /// (e.g. `"info"`, `"bithound=debug,sqlx=warn"`).
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_log_level() -> String {
    "info".into()
}
