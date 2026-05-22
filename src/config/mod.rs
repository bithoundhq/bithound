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

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use secrecy::SecretString;
use serde::Deserialize;
use sqlx::SqlitePool;
use thiserror::Error;
use uuid::Uuid;

use crate::shared::types::SidecarId;
use cli::Cli;
use collectors::CollectorDescriptorConfig;
use incidents::IncidentsConfig;
use notifications::{NotificationRuleConfig, NotificationTargetConfig, NotificationsConfig};
use runtime::RuntimeConfig;
use sidecar::SidecarConfig;
use storage::StorageConfig;
use targets::{BitcoinAuthConfig, BitcoinNodeConfig, HostConfig, LndNodeConfig};

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

    /// Full bootstrap. Parses the CLI, resolves the config path,
    /// reads + parses the TOML, applies `BITHOUND_*` env overrides,
    /// validates cross-references and env-var presence, reads secrets
    /// into `SecretString`, loads/generates the `SidecarId`, and opens
    /// the SQLite pool. Returns a `LoadedConfig` carrying every
    /// runtime-ready handle.
    ///
    /// Any failure exits the load with a `ConfigError`; the binary
    /// surfaces these as exit code 78 (`EX_CONFIG`).
    pub async fn load_from_args_and_env(cli: &Cli) -> Result<LoadedConfig, ConfigError> {
        let env_vars: HashMap<String, String> = std::env::vars().collect();
        Self::load_with_env(cli, &env_vars).await
    }

    /// Test seam for `load_from_args_and_env`. Production callers go
    /// through `load_from_args_and_env`; tests inject a synthetic env
    /// map so they can exercise overrides + secret resolution without
    /// poking real `std::env`.
    pub(crate) async fn load_with_env(
        cli: &Cli,
        env: &HashMap<String, String>,
    ) -> Result<LoadedConfig, ConfigError> {
        let config_path = resolve_config_path(cli.config.as_deref())?;

        let body = std::fs::read_to_string(&config_path).map_err(|e| ConfigError::ReadFile {
            path: config_path.clone(),
            source: e,
        })?;
        let mut raw: toml::Value = toml::from_str(&body)?;

        apply_env_overrides(&mut raw, env);
        secrets::reject_inline_secrets(&raw)?;

        let config: Config = raw.try_into()?;

        validate_cross_refs(&config)?;
        let needed = collect_env_refs(&config);
        validate_env_vars(&needed, env)?;
        let resolved = resolve_secrets(&needed, env)?;

        let sidecar_id = load_or_generate_sidecar_id(&config.sidecar.id_file)?;
        let pool = crate::storage::sqlite::open_pool(&config.storage.db_path).await?;

        Ok(LoadedConfig {
            config,
            secrets: resolved,
            sidecar_id,
            pool,
        })
    }
}

/// Runtime-ready bundle returned by `Config::load_from_args_and_env`.
/// Contains everything `runtime::run()` needs to spin up the sidecar.
#[derive(Debug)]
pub struct LoadedConfig {
    pub config: Config,
    pub secrets: ResolvedSecrets,
    pub sidecar_id: SidecarId,
    pub pool: SqlitePool,
}

/// Map of env-var-name → resolved `SecretString`. Built once during
/// `load_from_args_and_env` and handed to whichever runtime
/// component needs to dereference a `*_env` field on the config.
///
/// `SecretString` suppresses `Debug` / `Display` of the underlying
/// value, so logging a `ResolvedSecrets` won't leak credentials.
#[derive(Debug, Clone, Default)]
pub struct ResolvedSecrets {
    map: HashMap<String, SecretString>,
}

impl ResolvedSecrets {
    /// Look up a secret by env-var name. Returns `None` if the name
    /// wasn't referenced by any `*_env` field — callers should treat
    /// that as a programmer error since `validate_env_vars` runs
    /// upfront.
    pub fn get(&self, env_name: &str) -> Option<&SecretString> {
        self.map.get(env_name)
    }
}

// ---------------------------------------------------------------
// Step 2 — resolve config path
// ---------------------------------------------------------------

const DEFAULT_LOCAL_PATH: &str = "bithound.toml";
const DEFAULT_SYSTEM_PATH: &str = "/etc/bithound/bithound.toml";

fn resolve_config_path(cli_arg: Option<&Path>) -> Result<PathBuf, ConfigError> {
    if let Some(p) = cli_arg {
        if !p.exists() {
            return Err(ConfigError::Invalid(format!(
                "--config path {} does not exist",
                p.display()
            )));
        }
        return Ok(p.to_path_buf());
    }

    let local = PathBuf::from(DEFAULT_LOCAL_PATH);
    if local.exists() {
        return Ok(local);
    }

    let system = PathBuf::from(DEFAULT_SYSTEM_PATH);
    if system.exists() {
        return Ok(system);
    }

    Err(ConfigError::NotFound)
}

// ---------------------------------------------------------------
// Step 4 — apply env overrides for non-secret keys
// ---------------------------------------------------------------

/// Walks `env` looking for `BITHOUND_<section>__<key>` patterns. The
/// first `__` separates the top-level section from the key (sections
/// may contain single underscores, like `notification_rules`). The
/// override value is coerced to the type already in the TOML
/// document — int stays int, bool stays bool, otherwise string.
fn apply_env_overrides(raw: &mut toml::Value, env: &HashMap<String, String>) {
    let Some(root) = raw.as_table_mut() else {
        return;
    };

    for (name, value) in env {
        let Some(rest) = name.strip_prefix("BITHOUND_") else {
            continue;
        };
        let Some((section, key)) = rest.split_once("__") else {
            continue;
        };
        let section_lc = section.to_ascii_lowercase();
        let key_lc = key.to_ascii_lowercase();

        // Create the section if the TOML didn't define it. Without
        // this, overriding a key whose section is omitted (relying on
        // serde `default`) would silently no-op.
        let section_value = root
            .entry(section_lc.clone())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
        let Some(section_table) = section_value.as_table_mut() else {
            continue;
        };

        let existing = section_table.get(&key_lc);
        let coerced = coerce_env_value(value, existing);
        section_table.insert(key_lc, coerced);
    }
}

fn coerce_env_value(raw: &str, existing: Option<&toml::Value>) -> toml::Value {
    match existing {
        Some(toml::Value::Integer(_)) => raw
            .parse::<i64>()
            .map(toml::Value::Integer)
            .unwrap_or_else(|_| toml::Value::String(raw.to_string())),
        Some(toml::Value::Boolean(_)) => raw
            .parse::<bool>()
            .map(toml::Value::Boolean)
            .unwrap_or_else(|_| toml::Value::String(raw.to_string())),
        Some(toml::Value::Float(_)) => raw
            .parse::<f64>()
            .map(toml::Value::Float)
            .unwrap_or_else(|_| toml::Value::String(raw.to_string())),
        // Default and string-typed existing: treat as string. Best-effort
        // int parse so a value like `42` lands as Integer where the
        // schema expects one, even if the key was absent in the file.
        _ => raw
            .parse::<i64>()
            .map(toml::Value::Integer)
            .unwrap_or_else(|_| toml::Value::String(raw.to_string())),
    }
}

// ---------------------------------------------------------------
// Step 5 — validate cross-references
// ---------------------------------------------------------------

fn validate_cross_refs(config: &Config) -> Result<(), ConfigError> {
    let mut bitcoin_ids: std::collections::HashSet<&str> =
        config.bitcoin_nodes.iter().map(|n| n.id.as_str()).collect();
    let mut lnd_ids: std::collections::HashSet<&str> =
        config.lnd_nodes.iter().map(|n| n.id.as_str()).collect();
    let mut host_ids: std::collections::HashSet<&str> =
        config.hosts.iter().map(|h| h.id.as_str()).collect();

    if bitcoin_ids.len() != config.bitcoin_nodes.len() {
        return Err(ConfigError::Invalid(
            "duplicate id in [[bitcoin_nodes]]".into(),
        ));
    }
    if lnd_ids.len() != config.lnd_nodes.len() {
        return Err(ConfigError::Invalid("duplicate id in [[lnd_nodes]]".into()));
    }
    if host_ids.len() != config.hosts.len() {
        return Err(ConfigError::Invalid("duplicate id in [[hosts]]".into()));
    }

    for collector in &config.collectors {
        let (target_id, set) = match &collector.target {
            collectors::CollectorTargetConfig::BitcoinNode { id } => {
                (id.as_str(), &mut bitcoin_ids)
            }
            collectors::CollectorTargetConfig::LndNode { id } => (id.as_str(), &mut lnd_ids),
            collectors::CollectorTargetConfig::Host { id } => (id.as_str(), &mut host_ids),
        };
        if !set.contains(target_id) {
            return Err(ConfigError::Invalid(format!(
                "collector {:?} targets unknown id {:?}",
                collector.id, target_id
            )));
        }
    }

    let mut collector_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for c in &config.collectors {
        if !collector_ids.insert(c.id.as_str()) {
            return Err(ConfigError::Invalid(format!(
                "duplicate collector id {:?}",
                c.id
            )));
        }
    }

    let mut rule_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for r in &config.notification_rules {
        if !rule_ids.insert(r.id.as_str()) {
            return Err(ConfigError::Invalid(format!(
                "duplicate notification_rules.id {:?}",
                r.id
            )));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------
// Step 5/6 — collect *_env references, validate presence, resolve
// ---------------------------------------------------------------

/// Walks the typed config and returns every env-var name that needs
/// to be resolved. The list drives both the presence check (step 5)
/// and the secret read (step 6).
fn collect_env_refs(config: &Config) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();

    for node in &config.bitcoin_nodes {
        if let BitcoinAuthConfig::UserPass { password_env, .. } = &node.auth {
            out.push(password_env.clone());
        }
    }

    for node in &config.lnd_nodes {
        out.push(node.macaroon_env.clone());
    }

    if let Some(tg) = &config.notifications.telegram {
        out.push(tg.bot_token_env.clone());
    }

    for rule in &config.notification_rules {
        match &rule.target {
            NotificationTargetConfig::Telegram { .. } => {}
            NotificationTargetConfig::Discord { webhook_env, .. } => out.push(webhook_env.clone()),
            NotificationTargetConfig::Webhook { url_env } => out.push(url_env.clone()),
        }
    }

    out.sort();
    out.dedup();
    out
}

fn validate_env_vars(needed: &[String], env: &HashMap<String, String>) -> Result<(), ConfigError> {
    for name in needed {
        if !env.contains_key(name) {
            return Err(ConfigError::MissingEnv(name.clone()));
        }
    }
    Ok(())
}

fn resolve_secrets(
    needed: &[String],
    env: &HashMap<String, String>,
) -> Result<ResolvedSecrets, ConfigError> {
    let mut map: HashMap<String, SecretString> = HashMap::new();
    for name in needed {
        let value = env
            .get(name)
            .ok_or_else(|| ConfigError::MissingEnv(name.clone()))?;
        map.insert(name.clone(), SecretString::from(value.clone()));
    }
    Ok(ResolvedSecrets { map })
}

// ---------------------------------------------------------------
// Step 7 — read or generate SidecarId
// ---------------------------------------------------------------

fn load_or_generate_sidecar_id(path: &Path) -> Result<SidecarId, ConfigError> {
    if path.exists() {
        let raw = std::fs::read_to_string(path).map_err(|e| ConfigError::ReadFile {
            path: path.to_path_buf(),
            source: e,
        })?;
        let trimmed = raw.trim();
        let id = Uuid::parse_str(trimmed).map_err(|_| {
            ConfigError::Invalid(format!(
                "sidecar.id_file at {} does not contain a valid UUID",
                path.display()
            ))
        })?;
        return Ok(SidecarId(id));
    }

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let id = Uuid::now_v7();
    std::fs::write(path, format!("{}\n", id))?;
    Ok(SidecarId(id))
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
        let body = std::fs::read_to_string("examples/bithound.example.toml").expect("example.toml");
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
        assert_eq!(cfg.notification_rules.len(), 3);

        // The sample exercises all three target variants so any
        // schema change that drops one breaks here.
        let kinds: Vec<&'static str> = cfg
            .notification_rules
            .iter()
            .map(|r| match &r.target {
                notifications::NotificationTargetConfig::Telegram { .. } => "telegram",
                notifications::NotificationTargetConfig::Discord { .. } => "discord",
                notifications::NotificationTargetConfig::Webhook { .. } => "webhook",
            })
            .collect();
        assert!(kinds.contains(&"telegram"));
        assert!(kinds.contains(&"discord"));
        assert!(kinds.contains(&"webhook"));
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

    // ---- BTH-33 loader tests -------------------------------------

    /// A minimal config that round-trips cleanly. Helper used by
    /// every loader test that needs a valid baseline to perturb.
    fn baseline_toml(id_file: &Path, db_path: &Path) -> String {
        format!(
            r#"
[sidecar]
id_file = {id_file:?}

[storage]
db_path = {db_path:?}

[[bitcoin_nodes]]
id = "alice"
rpc_url = "http://127.0.0.1:8332"

[bitcoin_nodes.auth]
type = "user_pass"
user = "u"
password_env = "TEST_ALICE_PASSWORD"

[[collectors]]
id = "alice-rpc"
target = {{ type = "bitcoin_node", id = "alice" }}
integration = {{ type = "bitcoin_core_rpc", interval_seconds = 10 }}
instance_label = "alice"
"#
        )
    }

    fn write_toml(dir: &Path, body: &str) -> PathBuf {
        let p = dir.join("bithound.toml");
        std::fs::write(&p, body).expect("write toml");
        p
    }

    fn cli_for(path: &Path) -> Cli {
        Cli {
            config: Some(path.to_path_buf()),
            check_config: false,
            version: false,
        }
    }

    #[tokio::test]
    async fn missing_config_at_all_default_paths_fails_cleanly() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bogus = dir.path().join("does-not-exist.toml");
        let cli = Cli {
            config: Some(bogus),
            check_config: false,
            version: false,
        };
        let env: HashMap<String, String> = HashMap::new();
        let err = Config::load_with_env(&cli, &env).await.unwrap_err();
        match err {
            ConfigError::Invalid(msg) => assert!(msg.contains("does-not-exist"), "got: {msg}"),
            other => panic!("expected Invalid for missing path, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn cross_reference_error_when_collector_targets_unknown_node() {
        let dir = tempfile::tempdir().expect("tempdir");
        let id_file = dir.path().join("sidecar_id");
        let db = dir.path().join("bithound.db");

        let body = format!(
            r#"
[sidecar]
id_file = {id_file:?}

[storage]
db_path = {db:?}

[[bitcoin_nodes]]
id = "alice"
rpc_url = "http://127.0.0.1:8332"

[bitcoin_nodes.auth]
type = "user_pass"
user = "u"
password_env = "TEST_ALICE_PASSWORD"

[[collectors]]
id = "ghost-rpc"
target = {{ type = "bitcoin_node", id = "nonexistent-node" }}
integration = {{ type = "bitcoin_core_rpc", interval_seconds = 10 }}
instance_label = "ghost"
"#
        );
        let path = write_toml(dir.path(), &body);

        let mut env: HashMap<String, String> = HashMap::new();
        env.insert("TEST_ALICE_PASSWORD".into(), "secret".into());
        let err = Config::load_with_env(&cli_for(&path), &env)
            .await
            .unwrap_err();
        match err {
            ConfigError::Invalid(msg) => {
                assert!(msg.contains("nonexistent-node"), "got: {msg}");
                assert!(msg.contains("ghost-rpc"), "got: {msg}");
            }
            other => panic!("expected Invalid for bad cross-ref, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn env_override_changes_non_secret_key() {
        let dir = tempfile::tempdir().expect("tempdir");
        let id_file = dir.path().join("sidecar_id");
        let db = dir.path().join("bithound.db");
        let path = write_toml(dir.path(), &baseline_toml(&id_file, &db));

        let mut env: HashMap<String, String> = HashMap::new();
        env.insert("TEST_ALICE_PASSWORD".into(), "secret".into());
        env.insert("BITHOUND_RUNTIME__CHANNEL_CAPACITY".into(), "42".into());

        let loaded = Config::load_with_env(&cli_for(&path), &env)
            .await
            .expect("load");
        assert_eq!(loaded.config.runtime.channel_capacity, 42);
    }

    #[tokio::test]
    async fn missing_env_for_secret_reference_fails() {
        let dir = tempfile::tempdir().expect("tempdir");
        let id_file = dir.path().join("sidecar_id");
        let db = dir.path().join("bithound.db");
        let path = write_toml(dir.path(), &baseline_toml(&id_file, &db));

        // env does NOT contain TEST_ALICE_PASSWORD.
        let env: HashMap<String, String> = HashMap::new();
        let err = Config::load_with_env(&cli_for(&path), &env)
            .await
            .unwrap_err();
        match err {
            ConfigError::MissingEnv(name) => assert_eq!(name, "TEST_ALICE_PASSWORD"),
            other => panic!("expected MissingEnv, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn inline_password_is_rejected_through_loader() {
        let dir = tempfile::tempdir().expect("tempdir");
        let id_file = dir.path().join("sidecar_id");
        let db = dir.path().join("bithound.db");

        let body = format!(
            r#"
[sidecar]
id_file = {id_file:?}

[storage]
db_path = {db:?}

[[bitcoin_nodes]]
id = "alice"
rpc_url = "http://127.0.0.1:8332"

[bitcoin_nodes.auth]
type = "user_pass"
user = "u"
password = "hunter2"
"#
        );
        let path = write_toml(dir.path(), &body);

        let env: HashMap<String, String> = HashMap::new();
        let err = Config::load_with_env(&cli_for(&path), &env)
            .await
            .unwrap_err();
        assert!(matches!(err, ConfigError::InlineSecret(_)), "got {:?}", err);
    }

    #[tokio::test]
    async fn sidecar_id_is_generated_on_first_run_and_reused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let id_file = dir.path().join("sidecar_id");
        let db = dir.path().join("bithound.db");
        let path = write_toml(dir.path(), &baseline_toml(&id_file, &db));

        let mut env: HashMap<String, String> = HashMap::new();
        env.insert("TEST_ALICE_PASSWORD".into(), "secret".into());

        let first = Config::load_with_env(&cli_for(&path), &env)
            .await
            .expect("first load");
        assert!(id_file.exists(), "id_file should be generated");

        let second = Config::load_with_env(&cli_for(&path), &env)
            .await
            .expect("second load");
        assert_eq!(
            first.sidecar_id, second.sidecar_id,
            "sidecar id must be stable across restarts"
        );
    }

    #[tokio::test]
    async fn check_config_dump_does_not_leak_secret_values() {
        let dir = tempfile::tempdir().expect("tempdir");
        let id_file = dir.path().join("sidecar_id");
        let db = dir.path().join("bithound.db");
        let path = write_toml(dir.path(), &baseline_toml(&id_file, &db));

        let mut env: HashMap<String, String> = HashMap::new();
        env.insert("TEST_ALICE_PASSWORD".into(), "super-secret-value".into());

        let loaded = Config::load_with_env(&cli_for(&path), &env)
            .await
            .expect("load");

        // The Config-only dump that --check-config prints must
        // contain env-var *names* (so operators can verify which env
        // they need to set) but never the resolved secret value.
        let dump = format!("{:#?}", loaded.config);
        assert!(
            dump.contains("TEST_ALICE_PASSWORD"),
            "dump should reference the env var name"
        );
        assert!(
            !dump.contains("super-secret-value"),
            "dump must not contain resolved secret value"
        );

        // The bundled secrets handle never Debug-prints its values
        // either; SecretString blocks Debug at the type level.
        let secrets_dump = format!("{:?}", loaded.secrets);
        assert!(
            !secrets_dump.contains("super-secret-value"),
            "ResolvedSecrets must not leak via Debug"
        );
    }
}
