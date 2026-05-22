//! Configuration layer: parse `bithound.toml`, resolve env-var
//! secrets, validate cross-references, and hand the runtime a fully
//! built `Config`.
//!
//! Submodules are organized by concern; each defines the
//! `serde::Deserialize` shapes for its TOML block. `mod.rs` owns the
//! top-level `Config` aggregate, the error type, and the loader
//! entry point.

pub mod cli;
pub mod collectors;
pub mod incidents;
pub mod notifications;
pub mod runtime;
pub mod secrets;
pub mod sidecar;
pub mod storage;
pub mod targets;

use std::path::Path;

use serde::Deserialize;
use thiserror::Error;

use collectors::CollectorDescriptorConfig;
use incidents::IncidentsConfig;
use notifications::{NotificationRuleConfig, NotificationsConfig};
use runtime::RuntimeConfig;
use sidecar::SidecarConfig;
use storage::StorageConfig;
use targets::{BitcoinNodeConfig, HostConfig, LndNodeConfig};

/// Top-level configuration. Deserialized from `bithound.toml`.
///
/// Construct via `Config::from_toml_str` (pure parse) or
/// `Config::load_from_args_and_env` (full bootstrap including env
/// override, secrets resolution, validation, and sidecar-id +
/// SqlitePool acquisition).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub sidecar: SidecarConfig,
    pub storage: StorageConfig,

    #[serde(default)]
    pub runtime: RuntimeConfig,

    #[serde(default)]
    pub incidents: IncidentsConfig,

    #[serde(default)]
    pub bitcoin_nodes: Vec<BitcoinNodeConfig>,

    #[serde(default)]
    pub lnd_nodes: Vec<LndNodeConfig>,

    #[serde(default)]
    pub hosts: Vec<HostConfig>,

    #[serde(default)]
    pub collectors: Vec<CollectorDescriptorConfig>,

    #[serde(default)]
    pub notifications: NotificationsConfig,

    #[serde(default)]
    pub notification_rules: Vec<NotificationRuleConfig>,
}

impl Config {
    /// Pure-parse path. Rejects inline secrets up front, then runs
    /// serde over the document. The two-pass shape gives operators
    /// the specific "you wrote `foo.bar.password = …` in your config"
    /// error message instead of serde's generic "unknown field".
    pub fn from_toml_str(s: &str) -> Result<Self, ConfigError> {
        let raw: toml::Value = toml::from_str(s)?;
        secrets::reject_inline_secrets(&raw)?;
        let cfg: Config = raw.try_into()?;
        Ok(cfg)
    }

    /// Convenience wrapper around `from_toml_str` that reads the file
    /// itself. Used by the loader and by `--check-config`.
    pub fn from_toml_file(path: &Path) -> Result<Self, ConfigError> {
        let body = std::fs::read_to_string(path).map_err(|e| ConfigError::ReadFile {
            path: path.to_path_buf(),
            source: e,
        })?;
        Self::from_toml_str(&body)
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("read {path}: {source}")]
    ReadFile {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("config file not found at any default path (--config, ./bithound.toml, /etc/bithound/bithound.toml)")]
    NotFound,

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("toml parse: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("missing required env var: {0}")]
    MissingEnv(String),

    #[error("invalid: {0}")]
    Invalid(String),

    #[error("inline secret rejected at {0}")]
    InlineSecret(String),

    #[error("storage open failed: {0}")]
    StorageOpen(#[from] sqlx::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The example config bundled in `examples/bithound.example.toml`
    /// must always round-trip cleanly. If a schema change breaks the
    /// sample, the test fails — and the sample must be updated as
    /// part of that change.
    #[test]
    fn deserializes_example_toml() {
        let body =
            std::fs::read_to_string("examples/bithound.example.toml").expect("example.toml");
        let cfg = Config::from_toml_str(&body).expect("parse example");

        // Spot-check that every top-level section actually came in.
        assert_eq!(cfg.sidecar.log_level, "info");
        assert_eq!(cfg.runtime.channel_capacity, 1024);
        assert_eq!(cfg.bitcoin_nodes.len(), 1);
        assert_eq!(cfg.bitcoin_nodes[0].id, "btc-alice");
        assert_eq!(cfg.collectors.len(), 1);
        assert!(matches!(
            cfg.collectors[0].integration,
            collectors::IntegrationConfig::BitcoinCoreRpc {
                interval_seconds: 10
            }
        ));
        assert!(cfg.notifications.telegram.is_some());
        assert_eq!(cfg.notification_rules.len(), 1);
    }

    #[test]
    fn rejects_inline_password_with_clear_path() {
        let body = r#"
            [sidecar]
            id_file = "/tmp/x"

            [storage]
            db_path = "/tmp/x.db"

            [[bitcoin_nodes]]
            id = "a"
            rpc_url = "http://x"

            [bitcoin_nodes.auth]
            type = "user_pass"
            user = "u"
            password = "oops"
        "#;
        let err = Config::from_toml_str(body).unwrap_err();
        match err {
            ConfigError::InlineSecret(p) => {
                assert!(p.contains("password"), "got {p}");
            }
            other => panic!("expected InlineSecret, got {:?}", other),
        }
    }

    #[test]
    fn parse_error_points_at_offending_key() {
        let body = r#"
            [sidecar]
            id_file = "/tmp/x"
            this_field_does_not_exist = true

            [storage]
            db_path = "/tmp/x.db"
        "#;
        let err = Config::from_toml_str(body).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("this_field_does_not_exist"),
            "error should name the offending key; got: {msg}"
        );
    }
}
