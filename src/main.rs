// Many types are defined ahead of the runtime that will use them — the
// runtime wiring (Phase 10+) is what actually drives them. Allow
// dead_code crate-wide until that lands; CLAUDE.md flags this as
// expected for the current phase.
#![allow(dead_code)]

mod collectors;
mod config;
mod diagnostics;
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
use crate::storage::memory::notification_attempt_repository::MemoryNotificationAttemptRepository;
use crate::storage::sqlite::incident_repository::SqliteIncidentRepository;
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
    // BTH-52 will land a SqliteNotificationAttemptRepository; until
    // then the runtime uses the memory impl so audit rows don't
    // survive a restart. The trait surface is identical, so the
    // swap is a one-line change here.
    let attempts_repo: Arc<dyn NotificationAttemptRepository> =
        Arc::new(MemoryNotificationAttemptRepository::new());

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

    let notification_rules = build_notification_rules(&config.notification_rules, &secrets)?;
    let senders = build_senders(&config.notifications, &secrets, &http)?;

    let deps = runtime::RuntimeDeps {
        sidecar_id,
        polling_collectors: polling,
        subscription_collectors: subscription,
        rules: vec![], // Phase 11 wires concrete diagnostic rules.
        read_models,
        engine,
        notification_rules,
        senders,
        observation_store,
        incident_repo,
        attempts_repo,
        config: config.runtime,
    };

    runtime::run(deps).await?;
    Ok(())
}

fn build_notification_rules(
    rules: &[NotificationRuleConfig],
    secrets: &ResolvedSecrets,
) -> anyhow::Result<Vec<NotificationRule>> {
    let mut out: Vec<NotificationRule> = Vec::with_capacity(rules.len());
    for cfg in rules {
        let target = match &cfg.target {
            NotificationTargetConfig::Telegram { chat_id } => {
                NotificationTarget::Telegram(TelegramTarget {
                    chat_id: TelegramChatId(*chat_id),
                    parse_mode: TelegramParseMode::PlainText,
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

        out.push(NotificationRule {
            id: NotificationRuleId(cfg.id.clone()),
            name: NotificationRuleName(cfg.name.clone()),
            enabled: cfg.enabled,
            min_severity: map_severity(&cfg.min_severity),
            event_kinds: cfg
                .event_kinds
                .iter()
                .map(|k| IncidentKind(k.clone()))
                .collect(),
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

fn build_senders(
    notifications: &NotificationsConfig,
    secrets: &ResolvedSecrets,
    http: &reqwest::Client,
) -> anyhow::Result<NotifierSenders> {
    let webhook = WebhookSender::new(http.clone());

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
            // Parse-mode is per-target on TelegramTarget; the sink
            // config carries a default but TelegramSender itself
            // doesn't store one — leave that wiring to the per-rule
            // target in `build_notification_rules`.
            let _ = match cfg.parse_mode {
                TelegramParseModeConfig::Plain => TelegramParseMode::PlainText,
                TelegramParseModeConfig::Html => TelegramParseMode::Html,
                // MarkdownV2 isn't a runtime variant yet; fall back to Html
                // (most operators want some kind of formatting).
                TelegramParseModeConfig::MarkdownV2 => TelegramParseMode::Html,
            };
            Some(TelegramSender::new(token, http.clone()))
        }
        None => None,
    };

    // Discord and Webhook don't have a sink-wide secret yet (every
    // Discord rule carries its own webhook URL). Constructing senders
    // is unconditional.
    let discord = Some(DiscordSender::new(http.clone()));

    let _ = secrets; // keep the borrow alive for the early-return paths above
    Ok(NotifierSenders {
        webhook,
        telegram,
        discord,
    })
}
