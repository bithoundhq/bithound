//! Central pipeline consumer task.
//!
//! One task per sidecar drains the observation channel
//! and drives the read-model store + incident engine + notification
//! handoff. `&mut self` on the read-model store and the engine works
//! without locks because nothing else mutates them.
//!
//! Lifecycle events surface as Pending attempt rows
//! plus a `NotificationDispatch` sent to the worker. The consumer
//! never calls a sender directly.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use tokio::sync::{broadcast, mpsc};

use crate::diagnostics::traits::DiagnosticRule;
use crate::diagnostics::types::{DiagnosticContext, IncidentSignalDraft};
use crate::incidents::engine::{IncidentCommand, IncidentEngine};
use crate::incidents::events::IncidentEvent;
use crate::incidents::repository::IncidentRepository;
use crate::incidents::{IncidentLifecycleEvent, IncidentNotificationEventKind};
use crate::notifications::repository::NotificationAttemptRepository;
use crate::notifications::types::{
    NotificationAttempt, NotificationAttemptId, NotificationDeliveryStatus, NotificationMessage,
    NotificationRule, NotificationTarget, TargetKind,
};
use crate::observations::{Observation, ObservationBatch, ProbeResult};
use crate::read_models::store::ReadModelStore;
use crate::runtime::notification_worker::NotificationDispatch;
use crate::shared::types::EntityRef;
use crate::storage::traits::ObservationStore;

/// Drives the entire single-writer pipeline. Returns when the
/// upstream `mpsc::Receiver` is closed (every collector dropped its
/// sender) or when the shutdown broadcast fires; in the latter case
/// the consumer drains any batches still buffered in the channel
/// before exiting, so no observation already sent is lost.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    mut rx: mpsc::Receiver<ObservationBatch>,
    rules: Vec<Box<dyn DiagnosticRule>>,
    mut read_models: ReadModelStore,
    mut engine: IncidentEngine,
    notification_rules: Vec<NotificationRule>,
    notif_tx: mpsc::Sender<NotificationDispatch>,
    observation_store: Arc<dyn ObservationStore>,
    incident_repo: Arc<dyn IncidentRepository>,
    attempts_repo: Arc<dyn NotificationAttemptRepository>,
    mut shutdown: broadcast::Receiver<()>,
) {
    loop {
        tokio::select! {
            _ = shutdown.recv() => break,
            maybe_batch = rx.recv() => match maybe_batch {
                Some(batch) => {
                    process_batch(
                        batch,
                        &rules,
                        &mut read_models,
                        &mut engine,
                        &notification_rules,
                        &notif_tx,
                        observation_store.as_ref(),
                        incident_repo.as_ref(),
                        attempts_repo.as_ref(),
                    ).await;
                }
                None => return,
            }
        }
    }

    // Drain remaining batches that were already in the channel when
    // the shutdown signal fired. This keeps the audit trail honest:
    // every batch that reached the consumer either got processed or
    // recorded a failure trying.
    while let Some(batch) = rx.recv().await {
        process_batch(
            batch,
            &rules,
            &mut read_models,
            &mut engine,
            &notification_rules,
            &notif_tx,
            observation_store.as_ref(),
            incident_repo.as_ref(),
            attempts_repo.as_ref(),
        )
        .await;
    }
}

#[allow(clippy::too_many_arguments)]
async fn process_batch(
    batch: ObservationBatch,
    rules: &[Box<dyn DiagnosticRule>],
    read_models: &mut ReadModelStore,
    engine: &mut IncidentEngine,
    notification_rules: &[NotificationRule],
    notif_tx: &mpsc::Sender<NotificationDispatch>,
    observation_store: &dyn ObservationStore,
    incident_repo: &dyn IncidentRepository,
    attempts_repo: &dyn NotificationAttemptRepository,
) {
    let now = Utc::now();
    let monotonic_now = std::time::Instant::now();
    let collector_id = batch.collector.id.0.clone();

    let observations = observations_from_batch(&batch);
    let Some(subject) = observations.first().map(|o| o.subject.clone()) else {
        // Empty Ok batch: nothing to persist, nothing to apply, no
        // subject for the diagnostic context. Skip.
        return;
    };

    // ----- Persist and apply the batch observations ------------------
    for obs in &observations {
        if let Err(e) = observation_store.append(obs).await {
            tracing::error!(
                collector = %collector_id,
                error = ?e,
                "observation_store.append failed; skipping rest of batch",
            );
            return;
        }
        if let Err(e) = read_models.apply(obs) {
            tracing::warn!(
                collector = %collector_id,
                error = ?e,
                "read_models.apply failed; continuing",
            );
        }
    }

    // ----- Evaluate every rule against this batch's subject ----------
    let drafts = evaluate_rules(
        rules,
        read_models,
        &subject,
        now,
        monotonic_now,
        &collector_id,
    );

    // ----- Hand each draft to the engine, process emitted events ----
    for draft in drafts {
        let events = match engine.handle(IncidentCommand::RecordSignal(draft), now) {
            Ok(evs) => evs,
            Err(e) => {
                tracing::warn!(
                    collector = %collector_id,
                    error = ?e,
                    "engine.handle returned Err; skipping draft",
                );
                continue;
            }
        };

        for event in events {
            process_engine_event(
                event,
                read_models,
                notification_rules,
                notif_tx,
                observation_store,
                incident_repo,
                attempts_repo,
                now,
            )
            .await;
        }
    }
}

fn observations_from_batch(batch: &ObservationBatch) -> Vec<Observation> {
    // V0: the consumer only inspects observations that already carry
    // their own subject. For `ProbeResult::Failed` we surface the
    // `partial_observations` slice as-is; the engine's failure
    // signal will come from the next batch the collector produces
    // (which carries a health observation with a usable subject).
    match &batch.result {
        ProbeResult::Ok { observations } => observations.clone(),
        ProbeResult::Failed {
            partial_observations,
            ..
        } => partial_observations.clone(),
    }
}

fn evaluate_rules(
    rules: &[Box<dyn DiagnosticRule>],
    read_models: &ReadModelStore,
    subject: &EntityRef,
    now: DateTime<Utc>,
    monotonic_now: std::time::Instant,
    collector_id: &str,
) -> Vec<IncidentSignalDraft> {
    let mut drafts: Vec<IncidentSignalDraft> = Vec::new();
    for rule in rules {
        let ctx = DiagnosticContext {
            now,
            monotonic_now,
            subject,
            state: read_models,
            metrics: read_models,
            health: read_models,
            capabilities: read_models,
            heartbeats: read_models,
            signals: read_models,
        };
        // A panicking rule must not poison the cycle — the rest of
        // this batch's rules still get to run.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rule.evaluate(ctx)));
        match result {
            Ok(Ok(rule_drafts)) => drafts.extend(rule_drafts),
            Ok(Err(e)) => {
                tracing::warn!(
                    collector = %collector_id,
                    error = ?e,
                    "diagnostic rule failed; skipping",
                );
            }
            Err(_) => {
                tracing::error!(
                    collector = %collector_id,
                    "diagnostic rule panicked; skipping",
                );
            }
        }
    }
    drafts
}

#[allow(clippy::too_many_arguments)]
async fn process_engine_event(
    event: IncidentEvent,
    read_models: &mut ReadModelStore,
    notification_rules: &[NotificationRule],
    notif_tx: &mpsc::Sender<NotificationDispatch>,
    observation_store: &dyn ObservationStore,
    incident_repo: &dyn IncidentRepository,
    attempts_repo: &dyn NotificationAttemptRepository,
    now: DateTime<Utc>,
) {
    match event {
        IncidentEvent::SignalRecorded(obs) => {
            if let Err(e) = observation_store.append(&obs).await {
                tracing::error!(error = ?e, "observation_store.append (signal) failed");
            }
            if let Err(e) = read_models.apply(&obs) {
                tracing::warn!(error = ?e, "read_models.apply (signal) failed");
            }
        }
        IncidentEvent::IncidentTouched(incident) => {
            if let Err(e) = save_incident_with_retry(incident_repo, &incident).await {
                tracing::error!(
                    incident_id = ?incident.id,
                    error = ?e,
                    "incident_repo.save exhausted retries; continuing",
                );
            }
        }
        IncidentEvent::Lifecycle(lifecycle) => {
            dispatch_lifecycle(lifecycle, notification_rules, notif_tx, attempts_repo, now).await;
        }
        IncidentEvent::DraftBelowConfidenceFloor {
            kind,
            confidence,
            floor,
        } => {
            tracing::debug!(
                ?kind,
                ?confidence,
                ?floor,
                "draft below confidence floor; engine did not open an incident",
            );
        }
    }
}

/// V0 retry policy for incident persistence: 3 attempts total, with
/// 100ms and 500ms sleeps between them. Write-through-to-repo is the
/// invariant, so we retry, but we don't block the consumer
/// indefinitely. Exhaust = log + skip; the engine's in-memory state
/// will retry persistence on the next signal that touches this
/// incident.
async fn save_incident_with_retry(
    incident_repo: &dyn IncidentRepository,
    incident: &crate::incidents::Incident,
) -> Result<(), crate::incidents::repository::RepoError> {
    // Two waits between three attempts: try, sleep, try, sleep, try.
    const RETRY_DELAYS_MS: [u64; 2] = [100, 500];
    let mut last_err = None;
    for attempt_index in 0..=RETRY_DELAYS_MS.len() {
        match incident_repo.save(incident).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = Some(e);
                if let Some(&delay) = RETRY_DELAYS_MS.get(attempt_index) {
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
            }
        }
    }
    Err(last_err.expect("loop runs at least once"))
}

async fn dispatch_lifecycle(
    event: IncidentLifecycleEvent,
    notification_rules: &[NotificationRule],
    notif_tx: &mpsc::Sender<NotificationDispatch>,
    attempts_repo: &dyn NotificationAttemptRepository,
    now: DateTime<Utc>,
) {
    let message = compose_notification_message(&event, now);

    let mut attempts: Vec<NotificationAttemptId> = Vec::new();
    let mut targets: Vec<(NotificationAttemptId, NotificationTarget)> = Vec::new();

    for rule in notification_rules.iter().filter(|r| r.matches(&event)) {
        let attempt_id = NotificationAttemptId::new();
        let target = rule.target.clone();
        let (target_kind, target_summary) = TargetKind::summarize(&target);

        let pending = NotificationAttempt {
            id: attempt_id.clone(),
            rule_id: rule.id.clone(),
            incident_id: event.incident().id.clone(),
            lifecycle_kind: event.notification_kind(),
            target_kind,
            target_summary,
            status: NotificationDeliveryStatus::Pending,
            attempt_number: 1,
            parent_attempt_id: None,
            next_retry_at: None,
            outcome: None,
            external_ref: None,
            attempted_at: now,
            completed_at: None,
        };

        if let Err(e) = attempts_repo.insert_pending(&pending).await {
            tracing::error!(
                rule_id = ?rule.id,
                error = ?e,
                "attempts_repo.insert_pending failed; skipping this rule",
            );
            continue;
        }

        attempts.push(attempt_id.clone());
        targets.push((attempt_id, target));
    }

    if attempts.is_empty() {
        return;
    }

    let dispatch = NotificationDispatch {
        event,
        message,
        attempts,
        targets,
    };
    if notif_tx.send(dispatch).await.is_err() {
        tracing::error!("notification worker channel closed; lifecycle event dropped");
    }
}

fn compose_notification_message(
    event: &IncidentLifecycleEvent,
    now: DateTime<Utc>,
) -> NotificationMessage {
    let incident = event.incident();
    let verb = match event.notification_kind() {
        IncidentNotificationEventKind::Opened => "OPENED",
        IncidentNotificationEventKind::Escalated => "ESCALATED",
        IncidentNotificationEventKind::Resolved => "RESOLVED",
    };
    let title = format!(
        "{verb} [{severity:?}] {kind}",
        severity = incident.severity,
        kind = incident.kind.as_str(),
    );
    let summary = format!("incident {:?} on {:?}", incident.id, incident.subject);

    NotificationMessage {
        incident_lifecycle_event: event.clone(),
        title,
        summary,
        affected_component: None,
        diagnostic_summary: None,
        occurred_at: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration as ChronoDuration;
    use std::collections::BTreeMap;

    use crate::collectors::{CollectorRef, IntegrationKind};
    use crate::diagnostics::traits::DiagnosticRule;
    use crate::diagnostics::types::IncidentSignalDraft;
    use crate::incidents::kinds::KindRegistry;
    use crate::incidents::IncidentSeverity;
    use crate::notifications::targets::webhook::{WebhookMethod, WebhookTarget};
    use crate::notifications::types::{NotificationRuleId, NotificationRuleName};
    use crate::observations::{
        Attributes, Confidence, ObservationContext, ObservationOrigin, ObservationSource,
        ProbeResult, ProbeWindow, SignalName, SignalSeverity, SignalStatus, StateAtom,
    };
    use crate::read_models::store::ReadModelStoreConfig;
    use crate::shared::types::{
        BitcoinNodeId, CollectorId, EntityRef, ObservationBatchId, SidecarId,
    };
    use crate::storage::memory::incident_repository::MemoryIncidentRepository;
    use crate::storage::memory::notification_attempt_repository::MemoryNotificationAttemptRepository;
    use crate::storage::memory::observation_store::MemoryObservationStore;
    use uuid::Uuid;

    fn sidecar() -> SidecarId {
        SidecarId(Uuid::now_v7())
    }

    fn collector_ref() -> CollectorRef {
        CollectorRef {
            id: CollectorId("test-collector".into()),
            integration: IntegrationKind::BitcoinCoreRpc {
                interval: ChronoDuration::seconds(10),
            },
            instance_label: "test".into(),
        }
    }

    fn batch_with_state_obs(sidecar_id: &SidecarId, subject: EntityRef) -> ObservationBatch {
        let collector = collector_ref();
        let now = Utc::now();
        let ctx = ObservationContext {
            source: ObservationSource {
                sidecar_id: sidecar_id.clone(),
                collector: collector.clone(),
            },
            subject,
            observed_at: now,
            origin: ObservationOrigin::Collected,
        };
        let obs = Observation::transition(
            ctx,
            crate::observations::TransitionName::parse("test.transition").expect("valid"),
            StateAtom::String("a".into()),
            StateAtom::String("b".into()),
            None,
            Attributes(BTreeMap::new()),
        );
        ObservationBatch {
            id: ObservationBatchId::new(),
            collector,
            sidecar_id: sidecar_id.clone(),
            window: ProbeWindow::new(now, now).expect("window"),
            result: ProbeResult::Ok {
                observations: vec![obs],
            },
        }
    }

    fn signal_source(sidecar_id: &SidecarId) -> ObservationSource {
        ObservationSource {
            sidecar_id: sidecar_id.clone(),
            collector: collector_ref(),
        }
    }

    fn engine_with_builtins(sidecar_id: &SidecarId) -> IncidentEngine {
        IncidentEngine::new(
            KindRegistry::load(None).expect("built-in kinds"),
            sidecar_id.clone(),
            signal_source(sidecar_id),
            vec![],
        )
    }

    /// Registry with a single bitcoin.tip_lag kind registered so the
    /// AlwaysTipLagRule's drafts validate.
    fn engine_with_tip_lag(sidecar_id: &SidecarId) -> IncidentEngine {
        let user_toml = r#"
            [[kinds]]
            name = "bitcoin.tip_lag"
            allowed_subjects = ["BitcoinNode"]
            allows_dimension = false
            min_open_confidence = "Low"
        "#;
        let registry = KindRegistry::load_from_toml_strs("kinds = []", Some(user_toml))
            .expect("test kinds parse");
        IncidentEngine::new(
            registry,
            sidecar_id.clone(),
            signal_source(sidecar_id),
            vec![],
        )
    }

    fn rule_for_webhook() -> NotificationRule {
        NotificationRule {
            id: NotificationRuleId("rule".into()),
            name: NotificationRuleName("rule".into()),
            enabled: true,
            min_severity: IncidentSeverity::Info,
            event_kinds: vec![],
            target: NotificationTarget::Webhook(WebhookTarget {
                url: secrecy::SecretString::from("http://example.invalid".to_string()),
                method: WebhookMethod::Post,
                headers: vec![],
            }),
        }
    }

    // ---- BTH-35 ACs -------------------------------------------------

    #[tokio::test]
    async fn batch_appends_observations_with_empty_rules_no_events() {
        let sidecar_id = sidecar();
        let obs_store: Arc<MemoryObservationStore> = Arc::new(MemoryObservationStore::new());
        let incident_repo: Arc<MemoryIncidentRepository> =
            Arc::new(MemoryIncidentRepository::new());
        let attempts_repo: Arc<MemoryNotificationAttemptRepository> =
            Arc::new(MemoryNotificationAttemptRepository::new());

        let (tx, rx) = mpsc::channel::<ObservationBatch>(4);
        let (notif_tx, mut notif_rx) = mpsc::channel::<NotificationDispatch>(4);
        let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);

        let read_models = ReadModelStore::new(ReadModelStoreConfig::default());
        let engine = engine_with_builtins(&sidecar_id);

        let handle = tokio::spawn({
            let obs_store = Arc::clone(&obs_store) as Arc<dyn ObservationStore>;
            let incident_repo = Arc::clone(&incident_repo) as Arc<dyn IncidentRepository>;
            let attempts_repo =
                Arc::clone(&attempts_repo) as Arc<dyn NotificationAttemptRepository>;
            async move {
                run(
                    rx,
                    vec![],
                    read_models,
                    engine,
                    vec![],
                    notif_tx,
                    obs_store,
                    incident_repo,
                    attempts_repo,
                    shutdown_rx,
                )
                .await;
            }
        });

        tx.send(batch_with_state_obs(
            &sidecar_id,
            EntityRef::BitcoinNode(BitcoinNodeId("alice".into())),
        ))
        .await
        .unwrap();

        tokio::time::sleep(Duration::from_millis(80)).await;

        // The store has one observation now.
        use futures::StreamExt;
        let stream = obs_store
            .iter_since(DateTime::<Utc>::MIN_UTC)
            .await
            .expect("iter_since");
        let collected: Vec<_> = stream.collect().await;
        assert_eq!(collected.len(), 1, "single observation should be appended");

        assert!(
            notif_rx.try_recv().is_err(),
            "no rules → no notification dispatch",
        );

        let _ = shutdown_tx.send(());
        drop(tx);
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
    }

    // Stub rule that emits a single Active draft for a known built-in
    // incident kind (`bitcoin.tip_lag` from `well_known.rs`).
    struct AlwaysTipLagRule;
    impl DiagnosticRule for AlwaysTipLagRule {
        fn id(&self) -> &'static str {
            "always-tip-lag"
        }
        fn evaluate(&self, ctx: DiagnosticContext<'_>) -> anyhow::Result<Vec<IncidentSignalDraft>> {
            let kind =
                crate::incidents::IncidentKind::parse("bitcoin.tip_lag").expect("valid test kind");
            Ok(vec![IncidentSignalDraft {
                signal: SignalName::for_incident_kind(&kind),
                kind,
                subject: ctx.subject.clone(),
                dimension: None,
                severity: SignalSeverity::Warning,
                status: SignalStatus::Active,
                confidence: Confidence::High,
                evidence: vec![],
            }])
        }
    }

    #[tokio::test]
    async fn active_draft_produces_dispatch_with_open_lifecycle_event() {
        let sidecar_id = sidecar();
        let obs_store: Arc<MemoryObservationStore> = Arc::new(MemoryObservationStore::new());
        let incident_repo: Arc<MemoryIncidentRepository> =
            Arc::new(MemoryIncidentRepository::new());
        let attempts_repo: Arc<MemoryNotificationAttemptRepository> =
            Arc::new(MemoryNotificationAttemptRepository::new());

        let (tx, rx) = mpsc::channel::<ObservationBatch>(4);
        let (notif_tx, mut notif_rx) = mpsc::channel::<NotificationDispatch>(4);
        let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);

        let read_models = ReadModelStore::new(ReadModelStoreConfig::default());
        let engine = engine_with_tip_lag(&sidecar_id);
        let rules: Vec<Box<dyn DiagnosticRule>> = vec![Box::new(AlwaysTipLagRule)];

        let handle = tokio::spawn({
            let obs_store = Arc::clone(&obs_store) as Arc<dyn ObservationStore>;
            let incident_repo = Arc::clone(&incident_repo) as Arc<dyn IncidentRepository>;
            let attempts_repo =
                Arc::clone(&attempts_repo) as Arc<dyn NotificationAttemptRepository>;
            async move {
                run(
                    rx,
                    rules,
                    read_models,
                    engine,
                    vec![rule_for_webhook()],
                    notif_tx,
                    obs_store,
                    incident_repo,
                    attempts_repo,
                    shutdown_rx,
                )
                .await;
            }
        });

        tx.send(batch_with_state_obs(
            &sidecar_id,
            EntityRef::BitcoinNode(BitcoinNodeId("alice".into())),
        ))
        .await
        .unwrap();

        let dispatch = tokio::time::timeout(Duration::from_secs(2), notif_rx.recv())
            .await
            .expect("dispatch should arrive")
            .expect("dispatch is Some");

        assert!(matches!(dispatch.event, IncidentLifecycleEvent::Opened(_)));

        // One Pending attempt row inserted.
        let pending = attempts_repo
            .list_for_incident(&dispatch.event.incident().id)
            .await
            .expect("list");
        assert_eq!(pending.len(), 1, "exactly one pending attempt");
        assert_eq!(pending[0].status, NotificationDeliveryStatus::Pending);

        // One open incident saved.
        let open = incident_repo.load_open().await.expect("load_open");
        assert_eq!(open.len(), 1);

        let _ = shutdown_tx.send(());
        drop(tx);
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
    }

    struct PanickingRule;
    impl DiagnosticRule for PanickingRule {
        fn id(&self) -> &'static str {
            "panic"
        }
        fn evaluate(
            &self,
            _ctx: DiagnosticContext<'_>,
        ) -> anyhow::Result<Vec<IncidentSignalDraft>> {
            panic!("rule panic")
        }
    }

    #[tokio::test]
    async fn panicking_rule_does_not_poison_other_rules() {
        let sidecar_id = sidecar();
        let obs_store: Arc<MemoryObservationStore> = Arc::new(MemoryObservationStore::new());
        let incident_repo: Arc<MemoryIncidentRepository> =
            Arc::new(MemoryIncidentRepository::new());
        let attempts_repo: Arc<MemoryNotificationAttemptRepository> =
            Arc::new(MemoryNotificationAttemptRepository::new());

        let (tx, rx) = mpsc::channel::<ObservationBatch>(4);
        let (notif_tx, mut notif_rx) = mpsc::channel::<NotificationDispatch>(4);
        let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);

        let read_models = ReadModelStore::new(ReadModelStoreConfig::default());
        let engine = engine_with_tip_lag(&sidecar_id);
        let rules: Vec<Box<dyn DiagnosticRule>> =
            vec![Box::new(PanickingRule), Box::new(AlwaysTipLagRule)];

        let handle = tokio::spawn({
            let obs_store = Arc::clone(&obs_store) as Arc<dyn ObservationStore>;
            let incident_repo = Arc::clone(&incident_repo) as Arc<dyn IncidentRepository>;
            let attempts_repo =
                Arc::clone(&attempts_repo) as Arc<dyn NotificationAttemptRepository>;
            async move {
                run(
                    rx,
                    rules,
                    read_models,
                    engine,
                    vec![rule_for_webhook()],
                    notif_tx,
                    obs_store,
                    incident_repo,
                    attempts_repo,
                    shutdown_rx,
                )
                .await;
            }
        });

        tx.send(batch_with_state_obs(
            &sidecar_id,
            EntityRef::BitcoinNode(BitcoinNodeId("alice".into())),
        ))
        .await
        .unwrap();

        // The non-panicking rule must still fire, producing a
        // dispatch — proving the panic in the earlier rule didn't
        // poison the cycle.
        let dispatch = tokio::time::timeout(Duration::from_secs(2), notif_rx.recv())
            .await
            .expect("dispatch arrives despite earlier panic")
            .expect("dispatch is Some");
        assert!(matches!(dispatch.event, IncidentLifecycleEvent::Opened(_)));

        let _ = shutdown_tx.send(());
        drop(tx);
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
    }

    #[tokio::test]
    async fn shutdown_drains_remaining_batches_then_exits() {
        let sidecar_id = sidecar();
        let obs_store: Arc<MemoryObservationStore> = Arc::new(MemoryObservationStore::new());
        let incident_repo: Arc<MemoryIncidentRepository> =
            Arc::new(MemoryIncidentRepository::new());
        let attempts_repo: Arc<MemoryNotificationAttemptRepository> =
            Arc::new(MemoryNotificationAttemptRepository::new());

        let (tx, rx) = mpsc::channel::<ObservationBatch>(8);
        let (notif_tx, _notif_rx) = mpsc::channel::<NotificationDispatch>(4);
        let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);

        let read_models = ReadModelStore::new(ReadModelStoreConfig::default());
        let engine = engine_with_builtins(&sidecar_id);

        for _ in 0..3 {
            tx.send(batch_with_state_obs(
                &sidecar_id,
                EntityRef::BitcoinNode(BitcoinNodeId("alice".into())),
            ))
            .await
            .unwrap();
        }
        let _ = shutdown_tx.send(());

        let handle = tokio::spawn({
            let obs_store = Arc::clone(&obs_store) as Arc<dyn ObservationStore>;
            let incident_repo = Arc::clone(&incident_repo) as Arc<dyn IncidentRepository>;
            let attempts_repo =
                Arc::clone(&attempts_repo) as Arc<dyn NotificationAttemptRepository>;
            async move {
                run(
                    rx,
                    vec![],
                    read_models,
                    engine,
                    vec![],
                    notif_tx,
                    obs_store,
                    incident_repo,
                    attempts_repo,
                    shutdown_rx,
                )
                .await;
            }
        });

        drop(tx);

        let outcome = tokio::time::timeout(Duration::from_secs(3), handle).await;
        assert!(outcome.is_ok(), "consumer must exit after channel close");

        use futures::StreamExt;
        let collected: Vec<_> = obs_store
            .iter_since(DateTime::<Utc>::MIN_UTC)
            .await
            .expect("iter_since")
            .collect()
            .await;
        assert_eq!(
            collected.len(),
            3,
            "consumer must drain all 3 pre-buffered batches before exit",
        );
    }
}
