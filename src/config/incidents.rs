use std::path::PathBuf;

use serde::Deserialize;

/// Top-level `[incidents]` block. Currently only carries the optional
/// path to an operator-contributed kinds file.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncidentsConfig {
    /// External TOML listing operator-defined incident kinds and
    /// their defaults. The builtin kinds always load first; this
    /// file extends them.
    #[serde(default)]
    pub kinds_config_path: Option<PathBuf>,
}
