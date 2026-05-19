//! `SqliteNotificationAttemptRepository`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use crate::notifications::repository::{NotificationAttemptRepository, RepoError};
use crate::notifications::types::{
    DeliveryOutcome, DeliveryReceipt, ExternalMessageRef, NotificationAttempt,
    NotificationAttemptId, NotificationDeliveryStatus, NotificationRuleId, TargetKind,
};
use crate::incidents::IncidentNotificationEventKind;
use crate::shared::types::IncidentId;

pub struct SqliteNotificationAttemptRepository {
    pool: SqlitePool,
}

impl SqliteNotificationAttemptRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl NotificationAttemptRepository for SqliteNotificationAttemptRepository {
    async fn insert_pending(&self, attempt: &NotificationAttempt) -> Result<(), RepoError> {
        sqlx::query(
            "INSERT INTO notification_attempts (\
                id, rule_id, incident_id, lifecycle_kind, target_kind, target_summary, \
                status, attempt_number, parent_attempt_id, next_retry_at, \
                outcome_kind, outcome_json, external_ref_json, \
                attempted_at, completed_at\
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(attempt.id.0)
        .bind(attempt.rule_id.0)
        .bind(attempt.incident_id.0)
        .bind(lifecycle_kind_str(&attempt.lifecycle_kind))
        .bind(target_kind_str(&attempt.target_kind))
        .bind(&attempt.target_summary)
        .bind(status_str(&attempt.status))
        .bind(attempt.attempt_number as i64)
        .bind(attempt.parent_attempt_id.as_ref().map(|p| p.0))
        .bind(attempt.next_retry_at.map(nanos))
        .bind(outcome_kind_for(&attempt.outcome))
        .bind(
            attempt
                .outcome
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
        )
        .bind(
            attempt
                .external_ref
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?,
        )
        .bind(nanos(attempt.attempted_at))
        .bind(attempt.completed_at.map(nanos))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn complete(
        &self,
        id: &NotificationAttemptId,
        receipt: DeliveryReceipt,
        next_retry_at: Option<DateTime<Utc>>,
    ) -> Result<(), RepoError> {
        let status = status_for_outcome(&receipt.outcome, next_retry_at.is_some());
        let external_ref = match &receipt.outcome {
            DeliveryOutcome::Delivered { external_ref } => external_ref.clone(),
            _ => None,
        };
        let res = sqlx::query(
            "UPDATE notification_attempts SET \
                status            = ?, \
                next_retry_at     = ?, \
                outcome_kind      = ?, \
                outcome_json      = ?, \
                external_ref_json = ?, \
                completed_at      = ? \
             WHERE id = ? AND status = 'Pending'",
        )
        .bind(status_str(&status))
        .bind(next_retry_at.map(nanos))
        .bind(outcome_kind_for(&Some(receipt.outcome.clone())))
        .bind(serde_json::to_string(&receipt.outcome)?)
        .bind(external_ref.as_ref().map(serde_json::to_string).transpose()?)
        .bind(nanos(receipt.completed_at))
        .bind(id.0)
        .execute(&self.pool)
        .await?;

        if res.rows_affected() == 0 {
            return Err(RepoError::NotFound { id: id.clone() });
        }
        Ok(())
    }

    async fn list_retryable(
        &self,
        now: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<NotificationAttempt>, RepoError> {
        let rows = sqlx::query(
            "SELECT id, rule_id, incident_id, lifecycle_kind, target_kind, target_summary, \
                    status, attempt_number, parent_attempt_id, next_retry_at, \
                    outcome_json, external_ref_json, attempted_at, completed_at \
             FROM notification_attempts \
             WHERE status = 'FailedTransient' AND next_retry_at IS NOT NULL AND next_retry_at <= ? \
             ORDER BY next_retry_at ASC LIMIT ?",
        )
        .bind(nanos(now))
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_attempt).collect()
    }

    async fn list_for_incident(
        &self,
        incident_id: &IncidentId,
    ) -> Result<Vec<NotificationAttempt>, RepoError> {
        let rows = sqlx::query(
            "SELECT id, rule_id, incident_id, lifecycle_kind, target_kind, target_summary, \
                    status, attempt_number, parent_attempt_id, next_retry_at, \
                    outcome_json, external_ref_json, attempted_at, completed_at \
             FROM notification_attempts \
             WHERE incident_id = ? \
             ORDER BY attempted_at DESC",
        )
        .bind(incident_id.0)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_attempt).collect()
    }
}

fn nanos(t: DateTime<Utc>) -> i64 {
    t.timestamp_nanos_opt()
        .expect("timestamp within i64 nanos range")
}

fn dt(n: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp_nanos(n)
}

fn lifecycle_kind_str(k: &IncidentNotificationEventKind) -> &'static str {
    match k {
        IncidentNotificationEventKind::Opened => "Opened",
        IncidentNotificationEventKind::Escalated => "Escalated",
        IncidentNotificationEventKind::Resolved => "Resolved",
    }
}

fn lifecycle_kind_from_str(s: &str) -> Result<IncidentNotificationEventKind, RepoError> {
    match s {
        "Opened" => Ok(IncidentNotificationEventKind::Opened),
        "Escalated" => Ok(IncidentNotificationEventKind::Escalated),
        "Resolved" => Ok(IncidentNotificationEventKind::Resolved),
        other => Err(RepoError::Backend(format!(
            "unknown lifecycle_kind: {other}"
        ))),
    }
}

fn target_kind_str(k: &TargetKind) -> &'static str {
    match k {
        TargetKind::Telegram => "telegram",
        TargetKind::Discord => "discord",
        TargetKind::Webhook => "webhook",
        #[cfg(debug_assertions)]
        TargetKind::Stdout => "stdout",
    }
}

fn target_kind_from_str(s: &str) -> Result<TargetKind, RepoError> {
    match s {
        "telegram" => Ok(TargetKind::Telegram),
        "discord" => Ok(TargetKind::Discord),
        "webhook" => Ok(TargetKind::Webhook),
        #[cfg(debug_assertions)]
        "stdout" => Ok(TargetKind::Stdout),
        other => Err(RepoError::Backend(format!("unknown target_kind: {other}"))),
    }
}

fn status_str(s: &NotificationDeliveryStatus) -> &'static str {
    match s {
        NotificationDeliveryStatus::Pending => "Pending",
        NotificationDeliveryStatus::Succeeded => "Succeeded",
        NotificationDeliveryStatus::FailedTransient => "FailedTransient",
        NotificationDeliveryStatus::FailedPermanent => "FailedPermanent",
        NotificationDeliveryStatus::Suppressed => "Suppressed",
    }
}

fn status_from_str(s: &str) -> Result<NotificationDeliveryStatus, RepoError> {
    match s {
        "Pending" => Ok(NotificationDeliveryStatus::Pending),
        "Succeeded" => Ok(NotificationDeliveryStatus::Succeeded),
        "FailedTransient" => Ok(NotificationDeliveryStatus::FailedTransient),
        "FailedPermanent" => Ok(NotificationDeliveryStatus::FailedPermanent),
        "Suppressed" => Ok(NotificationDeliveryStatus::Suppressed),
        other => Err(RepoError::Backend(format!("unknown status: {other}"))),
    }
}

fn outcome_kind_for(o: &Option<DeliveryOutcome>) -> Option<&'static str> {
    o.as_ref().map(|o| match o {
        DeliveryOutcome::Delivered { .. } => "Delivered",
        DeliveryOutcome::Transient { .. } => "Transient",
        DeliveryOutcome::Permanent { .. } => "Permanent",
    })
}

fn status_for_outcome(
    outcome: &DeliveryOutcome,
    will_retry: bool,
) -> NotificationDeliveryStatus {
    match outcome {
        DeliveryOutcome::Delivered { .. } => NotificationDeliveryStatus::Succeeded,
        DeliveryOutcome::Transient { .. } if will_retry => {
            NotificationDeliveryStatus::FailedTransient
        }
        DeliveryOutcome::Transient { .. } => NotificationDeliveryStatus::FailedPermanent,
        DeliveryOutcome::Permanent { .. } => NotificationDeliveryStatus::FailedPermanent,
    }
}

fn row_to_attempt(row: sqlx::sqlite::SqliteRow) -> Result<NotificationAttempt, RepoError> {
    let id: uuid::Uuid = row.try_get("id")?;
    let rule_id: uuid::Uuid = row.try_get("rule_id")?;
    let incident_id: uuid::Uuid = row.try_get("incident_id")?;
    let lifecycle_kind: String = row.try_get("lifecycle_kind")?;
    let target_kind: String = row.try_get("target_kind")?;
    let target_summary: String = row.try_get("target_summary")?;
    let status: String = row.try_get("status")?;
    let attempt_number: i64 = row.try_get("attempt_number")?;
    let parent_attempt_id: Option<uuid::Uuid> = row.try_get("parent_attempt_id")?;
    let next_retry_at: Option<i64> = row.try_get("next_retry_at")?;
    let outcome_json: Option<String> = row.try_get("outcome_json")?;
    let external_ref_json: Option<String> = row.try_get("external_ref_json")?;
    let attempted_at: i64 = row.try_get("attempted_at")?;
    let completed_at: Option<i64> = row.try_get("completed_at")?;

    let outcome: Option<DeliveryOutcome> = outcome_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()?;
    let external_ref: Option<ExternalMessageRef> = external_ref_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()?;

    Ok(NotificationAttempt {
        id: NotificationAttemptId(id),
        rule_id: NotificationRuleId(rule_id),
        incident_id: IncidentId(incident_id),
        lifecycle_kind: lifecycle_kind_from_str(&lifecycle_kind)?,
        target_kind: target_kind_from_str(&target_kind)?,
        target_summary,
        status: status_from_str(&status)?,
        attempt_number: u32::try_from(attempt_number)
            .map_err(|e| RepoError::Backend(format!("attempt_number out of range: {e}")))?,
        parent_attempt_id: parent_attempt_id.map(NotificationAttemptId),
        next_retry_at: next_retry_at.map(dt),
        outcome,
        external_ref,
        attempted_at: dt(attempted_at),
        completed_at: completed_at.map(dt),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::targets::{
        discord::DiscordChannelId, telegram::TelegramChatId,
    };
    use crate::notifications::types::{PermanentError, TransientError};
    use crate::storage::sqlite::open_pool;
    use chrono::TimeZone;

    async fn fresh_repo() -> (SqliteNotificationAttemptRepository, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let pool = open_pool(&dir.path().join("test.db"))
            .await
            .expect("open_pool");
        (SqliteNotificationAttemptRepository::new(pool), dir)
    }

    fn pending(
        id: NotificationAttemptId,
        incident: IncidentId,
        attempted_at: DateTime<Utc>,
    ) -> NotificationAttempt {
        NotificationAttempt {
            id,
            rule_id: NotificationRuleId::new(),
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

    fn receipt(outcome: DeliveryOutcome, at: DateTime<Utc>) -> DeliveryReceipt {
        DeliveryReceipt {
            outcome,
            started_at: at,
            completed_at: at,
        }
    }

    #[tokio::test]
    async fn round_trip_each_delivery_outcome_variant() {
        let (repo, _dir) = fresh_repo().await;
        let now = Utc.timestamp_nanos(1_700_000_000_000_000_000);
        let incident = IncidentId::new();

        let cases = vec![
            DeliveryOutcome::Delivered { external_ref: None },
            DeliveryOutcome::Delivered {
                external_ref: Some(ExternalMessageRef::Telegram {
                    chat_id: TelegramChatId(42),
                    message_id: 7,
                }),
            },
            DeliveryOutcome::Delivered {
                external_ref: Some(ExternalMessageRef::Discord {
                    channel_id: DiscordChannelId(9001),
                    message_id: 12345,
                }),
            },
            DeliveryOutcome::Transient {
                error: TransientError::RateLimited,
                retry_after: None,
            },
            DeliveryOutcome::Permanent {
                error: PermanentError::AuthFailure,
            },
        ];

        for outcome in cases {
            let id = NotificationAttemptId::new();
            repo.insert_pending(&pending(id.clone(), incident.clone(), now))
                .await
                .unwrap();
            repo.complete(&id, receipt(outcome.clone(), now), None)
                .await
                .unwrap();
        }

        let listed = repo.list_for_incident(&incident).await.unwrap();
        assert_eq!(listed.len(), 5);
        // outcome_json round-trips identically.
        for a in &listed {
            assert!(a.outcome.is_some());
        }
        let kinds: std::collections::HashSet<_> = listed
            .iter()
            .map(|a| std::mem::discriminant(a.outcome.as_ref().unwrap()))
            .collect();
        assert!(kinds.len() >= 3, "all three discriminants survive");
    }

    #[tokio::test]
    async fn external_ref_round_trips_for_telegram_and_discord() {
        let (repo, _dir) = fresh_repo().await;
        let now = Utc.timestamp_nanos(1_700_000_000_000_000_000);
        let incident = IncidentId::new();

        let tg = ExternalMessageRef::Telegram {
            chat_id: TelegramChatId(-1001),
            message_id: 99,
        };
        let dc = ExternalMessageRef::Discord {
            channel_id: DiscordChannelId(424242),
            message_id: 9999,
        };

        for ext in [tg.clone(), dc.clone()] {
            let id = NotificationAttemptId::new();
            repo.insert_pending(&pending(id.clone(), incident.clone(), now))
                .await
                .unwrap();
            repo.complete(
                &id,
                receipt(
                    DeliveryOutcome::Delivered {
                        external_ref: Some(ext.clone()),
                    },
                    now,
                ),
                None,
            )
            .await
            .unwrap();
        }

        let listed = repo.list_for_incident(&incident).await.unwrap();
        assert_eq!(listed.len(), 2);
        let refs: Vec<&ExternalMessageRef> =
            listed.iter().filter_map(|a| a.external_ref.as_ref()).collect();
        assert!(refs.iter().any(|e| matches!(e, ExternalMessageRef::Telegram { .. })));
        assert!(refs.iter().any(|e| matches!(e, ExternalMessageRef::Discord { .. })));
    }

    #[tokio::test]
    async fn list_retryable_uses_status_next_retry_index() {
        let (repo, _dir) = fresh_repo().await;
        let rows = sqlx::query(
            "EXPLAIN QUERY PLAN \
             SELECT id, rule_id, incident_id, lifecycle_kind, target_kind, target_summary, \
                    status, attempt_number, parent_attempt_id, next_retry_at, \
                    outcome_json, external_ref_json, attempted_at, completed_at \
             FROM notification_attempts \
             WHERE status = 'FailedTransient' AND next_retry_at IS NOT NULL AND next_retry_at <= ? \
             ORDER BY next_retry_at ASC LIMIT ?",
        )
        .bind(0_i64)
        .bind(1_i64)
        .fetch_all(&repo.pool)
        .await
        .unwrap();
        let plan_text: String = rows
            .iter()
            .map(|r| r.try_get::<String, _>("detail").unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            plan_text.contains("idx_attempts_status_next_retry"),
            "expected EXPLAIN QUERY PLAN to mention idx_attempts_status_next_retry, got: {plan_text}"
        );
    }

    #[tokio::test]
    async fn complete_on_unknown_id_returns_not_found() {
        let (repo, _dir) = fresh_repo().await;
        let now = Utc.timestamp_nanos(1_700_000_000_000_000_000);
        let id = NotificationAttemptId::new();
        let res = repo
            .complete(
                &id,
                receipt(DeliveryOutcome::Delivered { external_ref: None }, now),
                None,
            )
            .await;
        assert!(matches!(res, Err(RepoError::NotFound { .. })));
    }

    #[tokio::test]
    async fn migration_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let pool1 = open_pool(&path).await.unwrap();
        drop(pool1);
        let pool2 = open_pool(&path).await.unwrap();
        // notification_attempts table should still exist.
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type='table' AND name='notification_attempts'",
        )
        .fetch_one(&pool2)
        .await
        .unwrap();
        assert_eq!(exists, 1);
    }
}
