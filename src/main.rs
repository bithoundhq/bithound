// Many types are defined ahead of the runtime that will use them — the
// runtime wiring (Phase 10+) is what actually drives them. Allow
// dead_code crate-wide until that lands; CLAUDE.md flags this as
// expected for the current phase.
#![allow(dead_code)]

mod api;
mod collectors;
mod config;
mod diagnostics;
mod domain_events;
mod incidents;
mod notifications;
mod observations;
mod read_models;
mod rpc;
mod runtime;
mod shared;
mod storage;

use std::sync::Arc;

use clap::Parser;

use crate::collectors::CollectorRef;
use crate::config::cli::Cli;
use crate::config::notifications::{
    NotificationRuleConfig, NotificationTargetConfig, NotificationsConfig, SeverityConfig,
    TelegramParseModeConfig,
};
use crate::config::{Config, LoadedConfig, ResolvedSecrets};
use crate::diagnostics::rules::bitcoin::{
    BitcoinNoPeersRule, BitcoinRpcUnreachableRule, BitcoinTipLagOrIbdStalledRule,
};
use crate::diagnostics::traits::DiagnosticRule;
use crate::incidents::kinds::KindRegistry;
use crate::incidents::repository::IncidentRepository;
use crate::incidents::{IncidentKind, IncidentSeverity};
use crate::notifications::repository::NotificationAttemptRepository;
use crate::notifications::targets::discord::{DiscordSender, DiscordTarget, DiscordThreadId};
use crate::notifications::targets::telegram::{
    TelegramChatId, TelegramParseMode, TelegramSender, TelegramTarget,
};
use crate::notifications::targets::webhook::{
    WebhookHeader, WebhookMethod, WebhookSender, WebhookTarget,
};
use crate::notifications::types::{
    NotificationRule, NotificationRuleId, NotificationRuleName, NotificationTarget,
};
use crate::observations::ObservationSource;
use crate::read_models::store::{ReadModelStore, ReadModelStoreConfig};
use crate::runtime::bootstrap;
use crate::runtime::notification_worker::NotifierSenders;
use crate::shared::types::CollectorId;
use crate::storage::sqlite::incident_repository::SqliteIncidentRepository;
use crate::storage::sqlite::notification_attempt_repository::SqliteNotificationAttemptRepository;
use crate::storage::sqlite::observation_store::SqliteObservationStore;
use crate::storage::traits::ObservationStore;

const EX_CONFIG: i32 = 78;

#[tokio::main]
async fn main() {
    if let Err(code) = run().await {
        std::process::exit(code);
    }
}

async fn run() -> Result<(), i32> {
    init_tracing();

    let cli = Cli::parse();

    if cli.version {
        println!("bithound {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let loaded = match Config::load_from_args_and_env(&cli).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(error = %e, "config load failed");
            eprintln!("config error: {}", e);
            return Err(EX_CONFIG);
        }
    };

    if cli.check_config {
        // Config carries env-var names, never resolved values.
        // ResolvedSecrets uses SecretString which suppresses Debug.
        println!("{:#?}", loaded.config);
        return Ok(());
    }

    if let Err(e) = boot_runtime(loaded).await {
        tracing::error!(error = %e, "runtime startup failed");
        eprintln!("startup error: {}", e);
        return Err(EX_CONFIG);
    }
    Ok(())
}

fn init_tracing() {
    // EnvFilter falls back to "info" if RUST_LOG is unset; the
    // [sidecar].log_level field is applied on top once we've parsed
    // the config (TODO: re-init the filter after config load — V0.1+).
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();
}

async fn boot_runtime(loaded: LoadedConfig) -> anyhow::Result<()> {
    let LoadedConfig {
        config,
        secrets,
        sidecar_id,
        pool,
    } = loaded;

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let registry = bootstrap::node_registry_from_config(&config, &secrets)?;
    let polling = bootstrap::build_polling_collectors(&config.collectors, &registry, &http)?;
    let subscription =
        bootstrap::build_subscription_collectors(&config.collectors, &registry, &http)?;

    let observation_store: Arc<dyn ObservationStore> =
        Arc::new(SqliteObservationStore::new(pool.clone()));
    let incident_repo: Arc<dyn IncidentRepository> =
        Arc::new(SqliteIncidentRepository::new(pool.clone()));
    let attempts_repo: Arc<dyn NotificationAttemptRepository> =
        Arc::new(SqliteNotificationAttemptRepository::new(pool.clone()));

    let read_models = ReadModelStore::new(ReadModelStoreConfig::default());
    let kinds = KindRegistry::load(config.incidents.kinds_config_path.as_deref())?;
    let open_incidents = incident_repo.load_open().await?;

    let signal_source = ObservationSource {
        sidecar_id: sidecar_id.clone(),
        collector: CollectorRef {
            id: CollectorId("bithound:engine".into()),
            integration: crate::collectors::IntegrationKind::BitcoinCoreRpc {
                interval: chrono::Duration::seconds(0),
            },
            instance_label: "engine".into(),
        },
    };
    let engine = crate::incidents::engine::IncidentEngine::new(
        kinds,
        sidecar_id.clone(),
        signal_source,
        open_incidents,
    );

    let notification_rules =
        build_notification_rules(&config.notification_rules, &config.notifications, &secrets)?;
    let senders = build_senders(&config.notifications, &secrets, &http)?;

    let rules: Vec<Box<dyn DiagnosticRule>> = vec![
        Box::new(BitcoinRpcUnreachableRule::new()),
        Box::new(BitcoinNoPeersRule::new()),
        Box::new(BitcoinTipLagOrIbdStalledRule::new()),
    ];

    let deps = runtime::RuntimeDeps {
        sidecar_id,
        polling_collectors: polling,
        subscription_collectors: subscription,
        rules,
        read_models,
        engine,
        notification_rules,
        senders,
        observation_store,
        incident_repo,
        attempts_repo,
        config: config.runtime,
        api_config: config.api,
        sidecar_version: env!("CARGO_PKG_VERSION"),
    };

    runtime::run(deps).await?;
    Ok(())
}

fn build_notification_rules(
    rules: &[NotificationRuleConfig],
    notifications: &NotificationsConfig,
    secrets: &ResolvedSecrets,
) -> anyhow::Result<Vec<NotificationRule>> {
    // `[notifications.telegram].parse_mode` is sink-wide for V0 —
    // every Telegram rule inherits it. If the operator omits the
    // telegram block entirely, default to PlainText (the safest
    // choice; HTML-escaped output of plain text is well-defined,
    // the reverse isn't).
    let telegram_parse_mode = notifications
        .telegram
        .as_ref()
        .map(|t| map_telegram_parse_mode(&t.parse_mode))
        .unwrap_or(TelegramParseMode::PlainText);

    let mut out: Vec<NotificationRule> = Vec::with_capacity(rules.len());
    for cfg in rules {
        let target = match &cfg.target {
            NotificationTargetConfig::Telegram { chat_id } => {
                NotificationTarget::Telegram(TelegramTarget {
                    chat_id: TelegramChatId(*chat_id),
                    parse_mode: telegram_parse_mode.clone(),
                })
            }
            NotificationTargetConfig::Discord {
                webhook_env,
                thread_id,
            } => {
                let webhook_url = secrets
                    .get(webhook_env)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "discord rule {:?} references unresolved env {webhook_env:?}",
                            cfg.id
                        )
                    })?
                    .clone();
                NotificationTarget::Discord(DiscordTarget {
                    webhook_url,
                    thread_id: thread_id.map(DiscordThreadId),
                    username_override: None,
                    avatar_url_override: None,
                })
            }
            NotificationTargetConfig::Webhook { url_env } => {
                let url = secrets
                    .get(url_env)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "webhook rule {:?} references unresolved env {url_env:?}",
                            cfg.id
                        )
                    })?
                    .clone();
                NotificationTarget::Webhook(WebhookTarget {
                    url,
                    method: WebhookMethod::Post,
                    headers: Vec::<WebhookHeader>::new(),
                })
            }
        };

        let mut event_kinds: Vec<IncidentKind> = Vec::with_capacity(cfg.event_kinds.len());
        for k in &cfg.event_kinds {
            event_kinds.push(IncidentKind::parse(k).map_err(|e| {
                anyhow::anyhow!(
                    "notification rule {id:?}: invalid event_kind {k:?}: {e}",
                    id = cfg.id
                )
            })?);
        }

        out.push(NotificationRule {
            id: NotificationRuleId(cfg.id.clone()),
            name: NotificationRuleName(cfg.name.clone()),
            enabled: cfg.enabled,
            min_severity: map_severity(&cfg.min_severity),
            event_kinds,
            target,
        });
    }
    Ok(out)
}

fn map_severity(s: &SeverityConfig) -> IncidentSeverity {
    match s {
        SeverityConfig::Info => IncidentSeverity::Info,
        SeverityConfig::Warning => IncidentSeverity::Warning,
        SeverityConfig::Critical => IncidentSeverity::Critical,
    }
}

/// Map the operator-supplied parse mode to the runtime enum. MarkdownV2
/// isn't a runtime variant in V0; it falls back to Html so the operator's
/// intent ("formatted, not plain") is preserved.
fn map_telegram_parse_mode(c: &TelegramParseModeConfig) -> TelegramParseMode {
    match c {
        TelegramParseModeConfig::Plain => TelegramParseMode::PlainText,
        TelegramParseModeConfig::Html => TelegramParseMode::Html,
        TelegramParseModeConfig::MarkdownV2 => TelegramParseMode::Html,
    }
}

fn build_senders(
    notifications: &NotificationsConfig,
    secrets: &ResolvedSecrets,
    http: &reqwest::Client,
) -> anyhow::Result<NotifierSenders> {
    let webhook = WebhookSender::new(http.clone());

    // `parse_mode` from the sink config is consumed by
    // `build_notification_rules` (it lives on the per-rule
    // TelegramTarget, not on TelegramSender). The sender only needs
    // the bot token.
    let telegram = match &notifications.telegram {
        Some(cfg) => {
            let token = secrets
                .get(&cfg.bot_token_env)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "[notifications.telegram].bot_token_env {:?} not resolved",
                        cfg.bot_token_env
                    )
                })?
                .clone();
            Some(TelegramSender::new(token, http.clone()))
        }
        None => None,
    };

    // Discord and Webhook don't have a sink-wide secret yet — each
    // rule carries its own URL. The senders are constructed
    // unconditionally so any rule can route through them.
    let discord = Some(DiscordSender::new(http.clone()));

    Ok(NotifierSenders {
        webhook,
        telegram,
        discord,
    })
}
