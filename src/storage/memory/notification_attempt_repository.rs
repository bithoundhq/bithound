//! In-memory `NotificationAttemptRepository` for tests.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use tokio::sync::Mutex;

use crate::notifications::repository::{NotificationAttemptRepository, RepoError};
use crate::notifications::types::{
    DeliveryOutcome, DeliveryReceipt, NotificationAttempt, NotificationAttemptId,
    NotificationDeliveryStatus,
};
use crate::shared::types::IncidentId;

#[derive(Default)]
pub struct MemoryNotificationAttemptRepository {
    inner: Mutex<HashMap<NotificationAttemptId, NotificationAttempt>>,
}

impl MemoryNotificationAttemptRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl NotificationAttemptRepository for MemoryNotificationAttemptRepository {
    async fn insert_pending(&self, attempt: &NotificationAttempt) -> Result<(), RepoError> {
        let mut guard = self.inner.lock().await;
        if guard.contains_key(&attempt.id) {
            return Err(RepoError::Conflict {
                id: attempt.id.clone(),
            });
        }
        guard.insert(attempt.id.clone(), attempt.clone());
        Ok(())
    }

    async fn complete(
        &self,
        id: &NotificationAttemptId,
        receipt: DeliveryReceipt,
        next_retry_at: Option<DateTime<Utc>>,
    ) -> Result<(), RepoError> {
        let mut guard = self.inner.lock().await;
        let attempt = guard
            .get_mut(id)
            .ok_or_else(|| RepoError::NotFound { id: id.clone() })?;
        attempt.status = status_for_outcome(&receipt.outcome, next_retry_at.is_some());
        attempt.outcome = Some(receipt.outcome.clone());
        attempt.external_ref = match &receipt.outcome {
            DeliveryOutcome::Delivered { external_ref } => external_ref.clone(),
            _ => None,
        };
        attempt.next_retry_at = next_retry_at;
        attempt.completed_at = Some(receipt.completed_at);
        Ok(())
    }

    async fn list_retryable(
        &self,
        now: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<NotificationAttempt>, RepoError> {
        let guard = self.inner.lock().await;
        let mut out: Vec<NotificationAttempt> = guard
            .values()
            .filter(|a| a.status == NotificationDeliveryStatus::FailedTransient)
            .filter(|a| a.next_retry_at.map(|t| t <= now).unwrap_or(false))
            .cloned()
            .collect();
        out.sort_by_key(|a| a.next_retry_at);
        out.truncate(limit as usize);
        Ok(out)
    }

    async fn list_for_incident(
        &self,
        incident_id: &IncidentId,
    ) -> Result<Vec<NotificationAttempt>, RepoError> {
        let guard = self.inner.lock().await;
        let mut out: Vec<NotificationAttempt> = guard
            .values()
            .filter(|a| &a.incident_id == incident_id)
            .cloned()
            .collect();
        out.sort_by(|a, b| b.attempted_at.cmp(&a.attempted_at));
        Ok(out)
    }
}

/// Terminal status follows the outcome, except a Transient with retries
/// remaining stays FailedTransient (V0.1+), and a Transient without
/// remaining retries is FailedPermanent (V0 always hits this branch).
fn status_for_outcome(outcome: &DeliveryOutcome, will_retry: bool) -> NotificationDeliveryStatus {
    match outcome {
        DeliveryOutcome::Delivered { .. } => NotificationDeliveryStatus::Succeeded,
        DeliveryOutcome::Transient { .. } if will_retry => {
            NotificationDeliveryStatus::FailedTransient
        }
        DeliveryOutcome::Transient { .. } => NotificationDeliveryStatus::FailedPermanent,
        DeliveryOutcome::Permanent { .. } => NotificationDeliveryStatus::FailedPermanent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::incidents::IncidentNotificationEventKind;
    use crate::notifications::targets::telegram::TelegramChatId;
    use crate::notifications::types::{
        ExternalMessageRef, NotificationRuleId, PermanentError, TargetKind, TransientError,
    };
    use chrono::TimeZone;

    fn pending(
        id: NotificationAttemptId,
        incident: IncidentId,
        attempted_at: DateTime<Utc>,
    ) -> NotificationAttempt {
        NotificationAttempt {
            id,
            rule_id: NotificationRuleId("test-rule".into()),
            incident_id: incident,
            lifecycle_kind: IncidentNotificationEventKind::Opened,
            target_kind: TargetKind::Webhook,
            target_summary: "webhook:host=example.com".into(),
            status: NotificationDeliveryStatus::Pending,
            attempt_number: 1,
            parent_attempt_id: None,
            next_retry_at: None,
            outcome: None,
            external_ref: None,
            attempted_at,
            completed_at: None,
        }
    }

    fn ok_receipt(at: DateTime<Utc>) -> DeliveryReceipt {
        DeliveryReceipt {
            outcome: DeliveryOutcome::Delivered {
                external_ref: Some(ExternalMessageRef::Telegram {
                    chat_id: TelegramChatId(42),
                    message_id: 7,
                }),
            },
            started_at: at,
            completed_at: at,
        }
    }

    fn transient_receipt(at: DateTime<Utc>) -> DeliveryReceipt {
        DeliveryReceipt {
            outcome: DeliveryOutcome::Transient {
                error: TransientError::RateLimited,
                retry_after: None,
            },
            started_at: at,
            completed_at: at,
        }
    }

    fn permanent_receipt(at: DateTime<Utc>) -> DeliveryReceipt {
        DeliveryReceipt {
            outcome: DeliveryOutcome::Permanent {
                error: PermanentError::AuthFailure,
            },
            started_at: at,
            completed_at: at,
        }
    }

    #[tokio::test]
    async fn insert_pending_then_complete_succeeded() {
        let repo = MemoryNotificationAttemptRepository::new();
        let at = Utc.timestamp_nanos(1_700_000_000_000_000_000);
        let id = NotificationAttemptId::new();
        let incident = IncidentId::new();
        repo.insert_pending(&pending(id.clone(), incident.clone(), at))
            .await
            .unwrap();
        repo.complete(&id, ok_receipt(at), None).await.unwrap();

        let listed = repo.list_for_incident(&incident).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].status, NotificationDeliveryStatus::Succeeded);
        assert!(listed[0].external_ref.is_some());
    }

    #[tokio::test]
    async fn complete_transient_without_retry_lands_failed_permanent() {
        // V0 (audit-only) path: every Transient outcome lands permanent
        // because there's no retry scheduler scheduling next_retry_at.
        let repo = MemoryNotificationAttemptRepository::new();
        let at = Utc.timestamp_nanos(1_700_000_000_000_000_000);
        let id = NotificationAttemptId::new();
        let incident = IncidentId::new();
        repo.insert_pending(&pending(id.clone(), incident, at))
            .await
            .unwrap();
        repo.complete(&id, transient_receipt(at), None)
            .await
            .unwrap();

        let listed = repo.list_retryable(at, 10).await.unwrap();
        assert!(listed.is_empty(), "no rows in FailedTransient under V0");
    }

    #[tokio::test]
    async fn list_retryable_respects_window() {
        let repo = MemoryNotificationAttemptRepository::new();
        let now = Utc.timestamp_nanos(1_700_000_000_000_000_000);
        let due = now - chrono::Duration::seconds(1);
        let future = now + chrono::Duration::seconds(60);

        let id_a = NotificationAttemptId::new();
        let id_b = NotificationAttemptId::new();
        let incident = IncidentId::new();
        repo.insert_pending(&pending(id_a.clone(), incident.clone(), now))
            .await
            .unwrap();
        repo.insert_pending(&pending(id_b.clone(), incident.clone(), now))
            .await
            .unwrap();
        // Simulate V0.1 scheduler: complete with `next_retry_at`.
        repo.complete(&id_a, transient_receipt(now), Some(due))
            .await
            .unwrap();
        repo.complete(&id_b, transient_receipt(now), Some(future))
            .await
            .unwrap();

        let due_now = repo.list_retryable(now, 10).await.unwrap();
        assert_eq!(due_now.len(), 1);
        assert_eq!(due_now[0].id, id_a);
    }

    #[tokio::test]
    async fn complete_permanent_clears_external_ref() {
        let repo = MemoryNotificationAttemptRepository::new();
        let at = Utc.timestamp_nanos(1_700_000_000_000_000_000);
        let id = NotificationAttemptId::new();
        let incident = IncidentId::new();
        repo.insert_pending(&pending(id.clone(), incident.clone(), at))
            .await
            .unwrap();
        repo.complete(&id, permanent_receipt(at), None)
            .await
            .unwrap();

        let listed = repo.list_for_incident(&incident).await.unwrap();
        assert_eq!(
            listed[0].status,
            NotificationDeliveryStatus::FailedPermanent
        );
        assert!(listed[0].external_ref.is_none());
    }
}
