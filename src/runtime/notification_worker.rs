//! Notification dispatch worker.
//!
//! Per ADR-N2: the consumer never calls senders directly. Lifecycle
//! events surface here as `NotificationDispatch` messages, the
//! worker drives each target's sender, and the matching
//! `NotificationAttempt` row is flipped from `Pending` to a terminal
//! status. If the worker dies mid-dispatch the row stays `Pending` —
//! that's the audit-trail guarantee.

use std::sync::Arc;

use tokio::sync::{broadcast, mpsc};

use crate::incidents::IncidentLifecycleEvent;
use crate::notifications::repository::NotificationAttemptRepository;
use crate::notifications::targets::discord::{DiscordPayload, DiscordSender};
use crate::notifications::targets::telegram::{TelegramPayload, TelegramSender};
use crate::notifications::targets::webhook::{WebhookPayload, WebhookSender};
use crate::notifications::types::{
    DeliveryOutcome, DeliveryReceipt, NotificationAttemptId, NotificationMessage,
    NotificationTarget, PermanentError,
};

/// Message the consumer sends to the worker for one lifecycle event.
/// Carries the resolved targets (including their `SecretString`
/// credentials) so the worker can call the matching sender. The
/// targets are paired with the Pending `NotificationAttemptId` rows
/// the consumer already INSERTed, so the worker knows which row to
/// UPDATE on completion.
#[derive(Debug, Clone)]
pub struct NotificationDispatch {
    pub event: IncidentLifecycleEvent,
    pub message: NotificationMessage,
    pub attempts: Vec<NotificationAttemptId>,
    pub targets: Vec<(NotificationAttemptId, NotificationTarget)>,
}

/// Holds the sender instances the worker can route to. The webhook
/// sender is always present (it has no operator-supplied secret of
/// its own — the URL is per-rule). Telegram is `Option` because the
/// bot token comes from `[notifications.telegram]`, which the
/// operator may omit. Discord is `Option` likewise, even though
/// every Discord secret today lives on the rule — keeping the slot
/// optional leaves room for sink-wide defaults later.
pub struct NotifierSenders {
    pub webhook: WebhookSender,
    pub telegram: Option<TelegramSender>,
    pub discord: Option<DiscordSender>,
}

impl NotifierSenders {
    pub async fn dispatch(
        &self,
        target: &NotificationTarget,
        message: &NotificationMessage,
    ) -> DeliveryReceipt {
        let started_at = chrono::Utc::now();
        match target {
            NotificationTarget::Webhook(t) => {
                let payload = WebhookPayload {
                    body: serde_json::json!({
                        "title": message.title,
                        "summary": message.summary,
                        "affected_component": message.affected_component,
                        "diagnostic_summary": message.diagnostic_summary,
                        "occurred_at": message.occurred_at.to_rfc3339(),
                    }),
                };
                self.webhook.send(t, &payload).await
            }
            NotificationTarget::Telegram(t) => {
                let Some(sender) = self.telegram.as_ref() else {
                    return not_configured_receipt(started_at, "telegram");
                };
                let payload = TelegramPayload {
                    text: format!("{}\n\n{}", message.title, message.summary),
                    parse_mode: t.parse_mode.clone(),
                    disable_notification: None,
                    reply_to_message_id: None,
                    reply_markup: None,
                };
                sender.send(t, &payload).await
            }
            NotificationTarget::Discord(t) => {
                let Some(sender) = self.discord.as_ref() else {
                    return not_configured_receipt(started_at, "discord");
                };
                let payload = DiscordPayload {
                    content: Some(format!("**{}**\n{}", message.title, message.summary)),
                    username: None,
                    avatar_url: None,
                    embeds: vec![],
                    allowed_mentions: None,
                };
                sender.send(t, &payload).await
            }
            #[cfg(debug_assertions)]
            NotificationTarget::Stdout => {
                eprintln!("[bithound stdout] {}\n{}", message.title, message.summary);
                DeliveryReceipt {
                    outcome: DeliveryOutcome::Delivered { external_ref: None },
                    started_at,
                    completed_at: chrono::Utc::now(),
                }
            }
        }
    }
}

fn not_configured_receipt(
    started_at: chrono::DateTime<chrono::Utc>,
    which: &'static str,
) -> DeliveryReceipt {
    tracing::warn!(
        sender = which,
        "rule targets {which} but the sink wasn't configured at startup; \
         marking attempt as Permanent::NotConfigured",
    );
    DeliveryReceipt {
        outcome: DeliveryOutcome::Permanent {
            error: PermanentError::NotConfigured,
        },
        started_at,
        completed_at: chrono::Utc::now(),
    }
}

/// The worker's main loop. Receives dispatches, fans out to senders,
/// flips the corresponding Pending row to terminal status.
///
/// V0 has no retry tick — that's V0.1+ work. If a sender returns
/// Transient, the row lands as `FailedTransient` and stays there
/// until the V0.1 scheduler picks it up. If the worker dies between
/// dispatch and complete, the row stays Pending forever; retention
/// sweeps preserve it as audit-trail.
pub async fn run(
    mut rx: mpsc::Receiver<NotificationDispatch>,
    attempts_repo: Arc<dyn NotificationAttemptRepository>,
    senders: NotifierSenders,
    mut shutdown: broadcast::Receiver<()>,
) {
    loop {
        tokio::select! {
            _ = shutdown.recv() => break,
            maybe_dispatch = rx.recv() => match maybe_dispatch {
                Some(dispatch) => process_dispatch(&dispatch, &senders, attempts_repo.as_ref()).await,
                None => return, // consumer is gone
            }
        }
    }
}

async fn process_dispatch(
    dispatch: &NotificationDispatch,
    senders: &NotifierSenders,
    attempts_repo: &dyn NotificationAttemptRepository,
) {
    for (attempt_id, target) in &dispatch.targets {
        let receipt = senders.dispatch(target, &dispatch.message).await;
        if let Err(e) = attempts_repo.complete(attempt_id, receipt, None).await {
            tracing::error!(
                attempt = ?attempt_id,
                error = ?e,
                "attempts_repo.complete failed; row stays Pending",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::time::Duration;
    use uuid::Uuid;

    use crate::incidents::{
        Incident, IncidentFingerprint, IncidentKind, IncidentLifecycleEvent, IncidentSeverity,
        IncidentStatus,
    };
    use crate::notifications::types::{
        NotificationAttempt, NotificationDeliveryStatus, NotificationRuleId,
        TargetKind,
    };
    use crate::shared::types::{BitcoinNodeId, EntityRef, IncidentId, SidecarId};
    use crate::storage::memory::notification_attempt_repository::MemoryNotificationAttemptRepository;

    fn senders_stdout_only() -> NotifierSenders {
        // No real bot token or webhook URL needed — every test
        // dispatch we generate targets stdout.
        NotifierSenders {
            webhook: WebhookSender::new(reqwest::Client::new()),
            telegram: None,
            discord: None,
        }
    }

    fn fake_incident(subject: EntityRef) -> Incident {
        let kind = IncidentKind("test.kind".into());
        let now = Utc::now();
        Incident {
            id: IncidentId(Uuid::now_v7()),
            fingerprint: IncidentFingerprint {
                subject: subject.clone(),
                kind: kind.clone(),
                dimension: None,
            },
            kind,
            subject,
            severity: IncidentSeverity::Warning,
            status: IncidentStatus::Open,
            opened_at: now,
            updated_at: now,
            resolved_at: None,
            signal_observation_ids: vec![],
            evidence: vec![],
            summary: "test".into(),
            evidence_summary: vec![],
        }
    }

    fn fake_message(event: &IncidentLifecycleEvent) -> NotificationMessage {
        NotificationMessage {
            incident_lifecycle_event: event.clone(),
            title: "test title".into(),
            summary: "test summary".into(),
            affected_component: None,
            diagnostic_summary: None,
            occurred_at: Utc::now(),
        }
    }

    fn pending_attempt(id: NotificationAttemptId, incident_id: IncidentId) -> NotificationAttempt {
        let now = Utc::now();
        NotificationAttempt {
            id,
            rule_id: NotificationRuleId("rule".into()),
            incident_id,
            lifecycle_kind: crate::incidents::IncidentNotificationEventKind::Opened,
            target_kind: TargetKind::Stdout,
            target_summary: "stdout".into(),
            status: NotificationDeliveryStatus::Pending,
            attempt_number: 1,
            parent_attempt_id: None,
            next_retry_at: None,
            outcome: None,
            external_ref: None,
            attempted_at: now,
            completed_at: None,
        }
    }

    #[tokio::test]
    async fn dispatch_flips_pending_row_to_terminal() {
        let attempts_repo: Arc<MemoryNotificationAttemptRepository> =
            Arc::new(MemoryNotificationAttemptRepository::new());

        let incident = fake_incident(EntityRef::BitcoinNode(BitcoinNodeId("alice".into())));
        let incident_id = incident.id.clone();
        let attempt_id = NotificationAttemptId::new();
        let pending = pending_attempt(attempt_id.clone(), incident_id.clone());
        attempts_repo.insert_pending(&pending).await.unwrap();

        let event = IncidentLifecycleEvent::Opened(incident);
        let dispatch = NotificationDispatch {
            event: event.clone(),
            message: fake_message(&event),
            attempts: vec![attempt_id.clone()],
            targets: vec![(attempt_id.clone(), NotificationTarget::Stdout)],
        };

        let (tx, rx) = mpsc::channel::<NotificationDispatch>(4);
        let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);

        let handle = tokio::spawn({
            let attempts_repo = Arc::clone(&attempts_repo) as Arc<dyn NotificationAttemptRepository>;
            async move {
                run(rx, attempts_repo, senders_stdout_only(), shutdown_rx).await;
            }
        });

        tx.send(dispatch).await.unwrap();
        // Give the worker a moment.
        tokio::time::sleep(Duration::from_millis(80)).await;

        let after = attempts_repo
            .list_for_incident(&incident_id)
            .await
            .expect("list");
        assert_eq!(after.len(), 1);
        assert_ne!(
            after[0].status,
            NotificationDeliveryStatus::Pending,
            "row must flip out of Pending after dispatch",
        );

        let _ = shutdown_tx.send(());
        drop(tx);
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
    }

    #[tokio::test]
    async fn worker_dies_mid_dispatch_leaves_row_pending() {
        // If we never get to deliver the dispatch to the worker, the
        // pre-inserted Pending row stays Pending — that's the audit
        // trail guarantee.
        let attempts_repo: Arc<MemoryNotificationAttemptRepository> =
            Arc::new(MemoryNotificationAttemptRepository::new());

        let incident = fake_incident(EntityRef::BitcoinNode(BitcoinNodeId("alice".into())));
        let incident_id = incident.id.clone();
        let attempt_id = NotificationAttemptId::new();
        let pending = pending_attempt(attempt_id.clone(), incident_id.clone());
        attempts_repo.insert_pending(&pending).await.unwrap();

        let (tx, rx) = mpsc::channel::<NotificationDispatch>(4);
        let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);

        let handle = tokio::spawn({
            let attempts_repo = Arc::clone(&attempts_repo) as Arc<dyn NotificationAttemptRepository>;
            async move {
                run(rx, attempts_repo, senders_stdout_only(), shutdown_rx).await;
            }
        });

        // Shut the worker down WITHOUT sending it a dispatch.
        let _ = shutdown_tx.send(());
        drop(tx);
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;

        let after = attempts_repo
            .list_for_incident(&incident_id)
            .await
            .expect("list");
        assert_eq!(after.len(), 1);
        assert_eq!(
            after[0].status,
            NotificationDeliveryStatus::Pending,
            "row must stay Pending when the worker never processed it",
        );
    }

    #[tokio::test]
    async fn shutdown_signal_exits_worker_within_five_seconds() {
        let attempts_repo: Arc<MemoryNotificationAttemptRepository> =
            Arc::new(MemoryNotificationAttemptRepository::new());

        let (_tx, rx) = mpsc::channel::<NotificationDispatch>(4);
        let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);

        let handle = tokio::spawn({
            let attempts_repo = Arc::clone(&attempts_repo) as Arc<dyn NotificationAttemptRepository>;
            async move {
                run(rx, attempts_repo, senders_stdout_only(), shutdown_rx).await;
            }
        });

        tokio::time::sleep(Duration::from_millis(20)).await;
        shutdown_tx.send(()).unwrap();

        let outcome = tokio::time::timeout(Duration::from_secs(5), handle).await;
        assert!(outcome.is_ok(), "worker must exit within 5s of shutdown");
    }
}
