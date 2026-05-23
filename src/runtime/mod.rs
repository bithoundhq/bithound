//! Runtime layer: spawns the per-collector task tree, the central
//! pipeline consumer, and the notification dispatch worker, then
//! drives shutdown.

pub mod bootstrap;
pub mod consumer;
pub mod notification_worker;
pub mod supervisor;

use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinSet;

use crate::api::ApiDeps;
use crate::collectors::traits::{PollingCollector, SubscriptionCollector};
use crate::config::api::ApiConfig;
use crate::config::runtime::RuntimeConfig;
use crate::diagnostics::traits::DiagnosticRule;
use crate::incidents::engine::IncidentEngine;
use crate::incidents::repository::IncidentRepository;
use crate::notifications::repository::NotificationAttemptRepository;
use crate::notifications::types::NotificationRule;
use crate::read_models::store::ReadModelStore;
use crate::runtime::notification_worker::NotifierSenders;
use crate::shared::types::SidecarId;
use crate::storage::traits::ObservationStore;

/// The whole bundle the runtime needs to spin up.
pub struct RuntimeDeps {
    pub sidecar_id: SidecarId,
    pub polling_collectors: Vec<Box<dyn PollingCollector>>,
    pub subscription_collectors: Vec<Box<dyn SubscriptionCollector>>,
    pub rules: Vec<Box<dyn DiagnosticRule>>,
    pub read_models: ReadModelStore,
    pub engine: IncidentEngine,
    pub notification_rules: Vec<NotificationRule>,
    pub senders: NotifierSenders,
    pub observation_store: Arc<dyn ObservationStore>,
    pub incident_repo: Arc<dyn IncidentRepository>,
    pub attempts_repo: Arc<dyn NotificationAttemptRepository>,
    pub config: RuntimeConfig,
    pub api_config: ApiConfig,
    pub sidecar_version: &'static str,
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("signal wiring: {0}")]
    Signal(String),
}

/// Spin everything up, run until SIGINT/SIGTERM, then drive a clean
/// bounded shutdown.
///
/// Flow on a shutdown signal:
///   1. The broadcast::Sender fires once.
///   2. Every collector task exits and drops its mpsc::Sender clone.
///   3. The consumer task drains any in-flight batches, then exits.
///   4. The worker task receives the broadcast and exits.
///   5. The whole join is wrapped in a deadline (default 30s, see
///      RuntimeConfig::shutdown_deadline_seconds) so a stuck task
///      can't pin the supervisor forever.
pub async fn run(deps: RuntimeDeps) -> Result<(), RuntimeError> {
    // Capture the sidecar's start time before any task spawn so
    // /health.uptime_seconds reports time-since-sidecar-start, not
    // time-since-API-task-start. The API task may be the last task
    // spawned (and may even be disabled entirely), so measuring its
    // age would give operators a misleading number.
    let started_at = std::time::Instant::now();
    let polling_count = deps.polling_collectors.len();
    let subscription_count = deps.subscription_collectors.len();
    let collectors: Vec<&dyn PollingCollector> =
        deps.polling_collectors.iter().map(|b| b.as_ref()).collect();
    tracing::info!(
        sidecar_id = %deps.sidecar_id.0,
        polling_collectors = polling_count,
        subscription_collectors = subscription_count,
        notification_rules = deps.notification_rules.len(),
        diagnostic_rules = deps.rules.len(),
        "bithound runtime starting",
    );
    for c in collectors {
        tracing::info!(
            collector = ?c.descriptor().id,
            integration = ?c.descriptor().integration,
            target = ?c.descriptor().target,
            "polling collector loaded",
        );
    }
    // Subscription collectors get logged the same way — but we
    // can't borrow them after handing them to the supervisor, so
    // do it now.
    for c in &deps.subscription_collectors {
        tracing::info!(
            collector = ?c.descriptor().id,
            integration = ?c.descriptor().integration,
            target = ?c.descriptor().target,
            "subscription collector loaded",
        );
    }

    // ----- Wire the channels -----------------------------------------
    let (obs_tx, obs_rx) =
        mpsc::channel::<crate::observations::ObservationBatch>(deps.config.channel_capacity);
    let (notif_tx, notif_rx) = mpsc::channel::<notification_worker::NotificationDispatch>(256);
    let (shutdown_tx, _) = broadcast::channel::<()>(8);

    // ----- Spawn the three task families -----------------------------
    let mut tasks: JoinSet<&'static str> = JoinSet::new();

    {
        let sidecar = deps.sidecar_id.clone();
        let shutdown_tx = shutdown_tx.clone();
        tasks.spawn(async move {
            supervisor::run(
                sidecar,
                deps.polling_collectors,
                deps.subscription_collectors,
                obs_tx,
                &shutdown_tx,
            )
            .await;
            "supervisor"
        });
    }

    {
        let shutdown_rx = shutdown_tx.subscribe();
        let observation_store = Arc::clone(&deps.observation_store);
        let incident_repo = Arc::clone(&deps.incident_repo);
        let attempts_repo = Arc::clone(&deps.attempts_repo);
        let rules = deps.rules;
        let read_models = deps.read_models;
        let engine = deps.engine;
        let notification_rules = deps.notification_rules;
        tasks.spawn(async move {
            consumer::run(
                obs_rx,
                rules,
                read_models,
                engine,
                notification_rules,
                notif_tx,
                observation_store,
                incident_repo,
                attempts_repo,
                shutdown_rx,
            )
            .await;
            "consumer"
        });
    }

    {
        let shutdown_rx = shutdown_tx.subscribe();
        let attempts_repo = Arc::clone(&deps.attempts_repo);
        let senders = deps.senders;
        tasks.spawn(async move {
            notification_worker::run(notif_rx, attempts_repo, senders, shutdown_rx).await;
            "notification_worker"
        });
    }

    if deps.api_config.enabled {
        let shutdown_rx = shutdown_tx.subscribe();
        let api_deps = ApiDeps {
            sidecar_id: deps.sidecar_id.clone(),
            sidecar_version: deps.sidecar_version,
            started_at,
            incident_repo: Arc::clone(&deps.incident_repo),
            observation_store: Arc::clone(&deps.observation_store),
            attempts_repo: Arc::clone(&deps.attempts_repo),
        };
        let bind = deps.api_config.bind;
        tasks.spawn(async move {
            if let Err(e) = crate::api::server::run(bind, api_deps, shutdown_rx).await {
                tracing::error!(error = ?e, "api server exited with error");
            }
            "api"
        });
    } else {
        tracing::info!("operator api disabled via [api].enabled = false");
    }

    // ----- Wait for SIGINT / SIGTERM ---------------------------------
    wait_for_shutdown_signal().await;
    tracing::info!("shutdown signal received; broadcasting to tasks");
    let _ = shutdown_tx.send(());

    // ----- Bounded join ----------------------------------------------
    let deadline = Duration::from_secs(deps.config.shutdown_deadline_seconds as u64);
    let result = tokio::time::timeout(deadline, async {
        while let Some(joined) = tasks.join_next().await {
            match joined {
                Ok(label) => tracing::info!(task = label, "task exited"),
                Err(e) => tracing::warn!(error = ?e, "task join error"),
            }
        }
    })
    .await;

    match result {
        Ok(()) => {
            tracing::info!("clean shutdown complete");
        }
        Err(_elapsed) => {
            tracing::warn!(
                deadline_secs = deps.config.shutdown_deadline_seconds,
                "shutdown deadline exceeded; aborting remaining tasks",
            );
            tasks.abort_all();
            while tasks.join_next().await.is_some() {}
        }
    }

    Ok(())
}

#[cfg(unix)]
async fn wait_for_shutdown_signal() {
    use tokio::signal::unix::{signal, SignalKind};
    let mut sigterm = match signal(SignalKind::terminate()) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = ?e, "could not install SIGTERM handler; relying on SIGINT only");
            // Fall back to SIGINT only.
            let _ = tokio::signal::ctrl_c().await;
            return;
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = sigterm.recv() => {},
    }
}

#[cfg(not(unix))]
async fn wait_for_shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::runtime::RuntimeConfig as Cfg;
    use crate::incidents::kinds::KindRegistry;
    use crate::notifications::targets::webhook::WebhookSender;
    use crate::observations::ObservationSource;
    use crate::read_models::store::ReadModelStoreConfig;
    use crate::shared::types::{CollectorId, SidecarId};
    use crate::storage::memory::incident_repository::MemoryIncidentRepository;
    use crate::storage::memory::notification_attempt_repository::MemoryNotificationAttemptRepository;
    use crate::storage::memory::observation_store::MemoryObservationStore;
    use uuid::Uuid;

    fn empty_deps() -> RuntimeDeps {
        let sidecar_id = SidecarId(Uuid::now_v7());
        let signal_source = ObservationSource {
            sidecar_id: sidecar_id.clone(),
            collector: crate::collectors::CollectorRef {
                id: CollectorId("internal".into()),
                integration: crate::collectors::IntegrationKind::BitcoinCoreRpc {
                    interval: chrono::Duration::seconds(60),
                },
                instance_label: "internal".into(),
            },
        };

        RuntimeDeps {
            sidecar_id: sidecar_id.clone(),
            polling_collectors: vec![],
            subscription_collectors: vec![],
            rules: vec![],
            read_models: ReadModelStore::new(ReadModelStoreConfig::default()),
            engine: IncidentEngine::new(
                KindRegistry::load(None).expect("builtins"),
                sidecar_id,
                signal_source,
                vec![],
            ),
            notification_rules: vec![],
            senders: NotifierSenders {
                webhook: WebhookSender::new(reqwest::Client::new()),
                telegram: None,
                discord: None,
            },
            observation_store: Arc::new(MemoryObservationStore::new()),
            incident_repo: Arc::new(MemoryIncidentRepository::new()),
            attempts_repo: Arc::new(MemoryNotificationAttemptRepository::new()),
            config: Cfg::default(),
            // Tests don't bind a TCP port — keeps the runtime under
            // test fast and avoids fighting with whatever else is on
            // the box.
            api_config: ApiConfig {
                enabled: false,
                ..ApiConfig::default()
            },
            sidecar_version: "test-0.0.0",
        }
    }

    /// Sending SIGINT (via raise()) triggers the runtime's clean
    /// shutdown path. The exit must happen well within the 30s
    /// deadline — we cap the test at 5s.
    #[tokio::test]
    async fn ctrl_c_triggers_clean_shutdown_within_five_seconds() {
        let deps = empty_deps();
        let handle = tokio::spawn(async move { run(deps).await });

        // Give the runtime a moment to install signal handlers.
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Raise SIGINT to ourselves.
        unsafe {
            libc_raise_sigint();
        }

        let outcome = tokio::time::timeout(Duration::from_secs(5), handle).await;
        assert!(
            outcome.is_ok(),
            "runtime must exit within 5s of SIGINT (default deadline is 30s)",
        );
    }

    // We deliberately link libc only in the test build to keep the
    // production binary's dep tree clean.
    #[cfg(unix)]
    unsafe fn libc_raise_sigint() {
        // libc isn't a direct dep yet — use the tokio-bundled libc
        // via the kill_self std workaround.
        // SAFETY: kill() is a syscall; sending SIGINT to ourselves is
        // exactly what a Ctrl-C would do.
        extern "C" {
            fn raise(sig: i32) -> i32;
        }
        const SIGINT: i32 = 2;
        let _ = raise(SIGINT);
    }
}
