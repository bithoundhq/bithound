//! `SqliteNotificationAttemptRepository` per ADR-P3 §§P3.2, P3.3.
//!
//! Rows move through the state machine in two writes: an INSERT at
//! `status = Pending` followed by exactly one UPDATE to a terminal
//! status. Retries (V0.1+) produce *new* rows pointing back at the
//! original via `parent_attempt_id`; this impl never UPDATEs a row to
//! a different terminal status.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use uuid::Uuid;

use crate::incidents::IncidentNotificationEventKind;
use crate::notifications::repository::{NotificationAttemptRepository, RepoError};
use crate::notifications::types::{
    DeliveryOutcome, DeliveryReceipt, ExternalMessageRef, NotificationAttempt,
    NotificationAttemptId, NotificationDeliveryStatus, NotificationRuleId, TargetKind,
};
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
        let (outcome_kind, outcome_json) = serialize_outcome(attempt.outcome.as_ref())?;
        let external_ref_json = serialize_external_ref(attempt.external_ref.as_ref())?;

        sqlx::query(
            "INSERT INTO notification_attempts (\
                id, rule_id, incident_id, lifecycle_kind, \
                target_kind, target_summary, \
                status, attempt_number, parent_attempt_id, next_retry_at, \
                outcome_kind, outcome_json, external_ref_json, \
                attempted_at, completed_at\
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(attempt.id.0)
        .bind(rule_id_blob(&attempt.rule_id))
        .bind(attempt.incident_id.0)
        .bind(lifecycle_kind_str(&attempt.lifecycle_kind))
        .bind(target_kind_str(&attempt.target_kind))
        .bind(&attempt.target_summary)
        .bind(status_str(&attempt.status))
        .bind(attempt.attempt_number as i64)
        .bind(attempt.parent_attempt_id.as_ref().map(|p| p.0))
        .bind(attempt.next_retry_at.map(nanos))
        .bind(outcome_kind)
        .bind(outcome_json)
        .bind(external_ref_json)
        .bind(nanos(attempt.attempted_at))
        .bind(attempt.completed_at.map(nanos))
        .execute(&self.pool)
        .await
        .map_err(map_insert_error(attempt.id.clone()))?;
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
        let outcome = Some(receipt.outcome);
        let (outcome_kind, outcome_json) = serialize_outcome(outcome.as_ref())?;
        let external_ref_json = serialize_external_ref(external_ref.as_ref())?;

        let result = sqlx::query(
            "UPDATE notification_attempts \
             SET status = ?, \
                 outcome_kind = ?, \
                 outcome_json = ?, \
                 external_ref_json = ?, \
                 next_retry_at = ?, \
                 completed_at = ? \
             WHERE id = ?",
        )
        .bind(status_str(&status))
        .bind(outcome_kind)
        .bind(outcome_json)
        .bind(external_ref_json)
        .bind(next_retry_at.map(nanos))
        .bind(nanos(receipt.completed_at))
        .bind(id.0)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
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
            "SELECT id, rule_id, incident_id, lifecycle_kind, \
                    target_kind, target_summary, \
                    status, attempt_number, parent_attempt_id, next_retry_at, \
                    outcome_kind, outcome_json, external_ref_json, \
                    attempted_at, completed_at \
             FROM notification_attempts \
             WHERE status = ? AND next_retry_at IS NOT NULL AND next_retry_at <= ? \
             ORDER BY next_retry_at ASC \
             LIMIT ?",
        )
        .bind(status_str(&NotificationDeliveryStatus::FailedTransient))
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
            "SELECT id, rule_id, incident_id, lifecycle_kind, \
                    target_kind, target_summary, \
                    status, attempt_number, parent_attempt_id, next_retry_at, \
                    outcome_kind, outcome_json, external_ref_json, \
                    attempted_at, completed_at \
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

fn nanos_to_dt(n: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp_nanos(n)
}

/// `NotificationRuleId` carries an operator-supplied slug, not a UUID.
/// The DDL types the column as `BLOB` (per the ADR), so we round-trip
/// the slug as UTF-8 bytes — SQLite STRICT happily takes a BLOB
/// containing arbitrary bytes and the read path decodes the same way.
fn rule_id_blob(id: &NotificationRuleId) -> Vec<u8> {
    id.0.as_bytes().to_vec()
}

fn rule_id_from_blob(b: Vec<u8>) -> Result<NotificationRuleId, RepoError> {
    String::from_utf8(b)
        .map(NotificationRuleId)
        .map_err(|e| RepoError::Backend(format!("rule_id is not utf-8: {e}")))
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
            "unknown lifecycle_kind in row: {other}"
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
        other => Err(RepoError::Backend(format!(
            "unknown target_kind in row: {other}"
        ))),
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
        other => Err(RepoError::Backend(format!(
            "unknown status in row: {other}"
        ))),
    }
}

fn outcome_kind_str(o: &DeliveryOutcome) -> &'static str {
    match o {
        DeliveryOutcome::Delivered { .. } => "Delivered",
        DeliveryOutcome::Transient { .. } => "Transient",
        DeliveryOutcome::Permanent { .. } => "Permanent",
    }
}

fn serialize_outcome(
    outcome: Option<&DeliveryOutcome>,
) -> Result<(Option<&'static str>, Option<String>), RepoError> {
    match outcome {
        Some(o) => Ok((Some(outcome_kind_str(o)), Some(serde_json::to_string(o)?))),
        None => Ok((None, None)),
    }
}

fn serialize_external_ref(
    external_ref: Option<&ExternalMessageRef>,
) -> Result<Option<String>, RepoError> {
    match external_ref {
        Some(r) => Ok(Some(serde_json::to_string(r)?)),
        None => Ok(None),
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

fn map_insert_error(id: NotificationAttemptId) -> impl FnOnce(sqlx::Error) -> RepoError {
    move |err| match &err {
        sqlx::Error::Database(db_err) if db_err.is_unique_violation() => RepoError::Conflict { id },
        _ => RepoError::Backend(err.to_string()),
    }
}

fn row_to_attempt(row: sqlx::sqlite::SqliteRow) -> Result<NotificationAttempt, RepoError> {
    let id: Uuid = row.try_get("id")?;
    let rule_id_bytes: Vec<u8> = row.try_get("rule_id")?;
    let incident_id: Uuid = row.try_get("incident_id")?;
    let lifecycle: String = row.try_get("lifecycle_kind")?;
    let target_kind: String = row.try_get("target_kind")?;
    let target_summary: String = row.try_get("target_summary")?;
    let status: String = row.try_get("status")?;
    let attempt_number: i64 = row.try_get("attempt_number")?;
    let parent_attempt_id: Option<Uuid> = row.try_get("parent_attempt_id")?;
    let next_retry_at: Option<i64> = row.try_get("next_retry_at")?;
    let outcome_json: Option<String> = row.try_get("outcome_json")?;
    let external_ref_json: Option<String> = row.try_get("external_ref_json")?;
    let attempted_at: i64 = row.try_get("attempted_at")?;
    let completed_at: Option<i64> = row.try_get("completed_at")?;

    let outcome: Option<DeliveryOutcome> = match outcome_json {
        Some(j) => Some(serde_json::from_str(&j)?),
        None => None,
    };
    let external_ref: Option<ExternalMessageRef> = match external_ref_json {
        Some(j) => Some(serde_json::from_str(&j)?),
        None => None,
    };

    if attempt_number < 0 {
        return Err(RepoError::Backend(format!(
            "attempt_number is negative: {attempt_number}"
        )));
    }

    Ok(NotificationAttempt {
        id: NotificationAttemptId(id),
        rule_id: rule_id_from_blob(rule_id_bytes)?,
        incident_id: IncidentId(incident_id),
        lifecycle_kind: lifecycle_kind_from_str(&lifecycle)?,
        target_kind: target_kind_from_str(&target_kind)?,
        target_summary,
        status: status_from_str(&status)?,
        attempt_number: attempt_number as u32,
        parent_attempt_id: parent_attempt_id.map(NotificationAttemptId),
        next_retry_at: next_retry_at.map(nanos_to_dt),
        outcome,
        external_ref,
        attempted_at: nanos_to_dt(attempted_at),
        completed_at: completed_at.map(nanos_to_dt),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::targets::discord::DiscordChannelId;
    use crate::notifications::targets::telegram::TelegramChatId;
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

    fn delivered_receipt(at: DateTime<Utc>) -> DeliveryReceipt {
        DeliveryReceipt {
            outcome: DeliveryOutcome::Delivered {
                external_ref: Some(ExternalMessageRef::Telegram {
                    chat_id: TelegramChatId(-1001234567890),
                    message_id: 42,
                }),
            },
            started_at: at,
            completed_at: at,
        }
    }

    fn delivered_discord(at: DateTime<Utc>) -> DeliveryReceipt {
        DeliveryReceipt {
            outcome: DeliveryOutcome::Delivered {
                external_ref: Some(ExternalMessageRef::Discord {
                    channel_id: DiscordChannelId(987654321),
                    message_id: 1234567890,
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
                retry_after: Some(chrono::Duration::seconds(30)),
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
        let (repo, _dir) = fresh_repo().await;
        let at = Utc.timestamp_nanos(1_700_000_000_000_000_000);
        let id = NotificationAttemptId::new();
        let incident = IncidentId::new();

        repo.insert_pending(&pending(id.clone(), incident.clone(), at))
            .await
            .unwrap();
        repo.complete(&id, delivered_receipt(at), None)
            .await
            .unwrap();

        let listed = repo.list_for_incident(&incident).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].status, NotificationDeliveryStatus::Succeeded);
        assert!(matches!(
            listed[0].outcome,
            Some(DeliveryOutcome::Delivered { .. })
        ));
        assert!(matches!(
            listed[0].external_ref,
            Some(ExternalMessageRef::Telegram { .. })
        ));
        assert_eq!(listed[0].completed_at, Some(at));
    }

    #[tokio::test]
    async fn insert_pending_then_complete_failed_permanent() {
        let (repo, _dir) = fresh_repo().await;
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

    #[tokio::test]
    async fn complete_transient_without_retry_lands_failed_permanent() {
        // V0 (audit-only) path: every Transient outcome lands permanent
        // because the V0 scheduler doesn't set `next_retry_at`.
        let (repo, _dir) = fresh_repo().await;
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
    async fn list_retryable_respects_window_and_order() {
        let (repo, _dir) = fresh_repo().await;
        let now = Utc.timestamp_nanos(1_700_000_000_000_000_000);
        let due_early = now - chrono::Duration::seconds(60);
        let due_later = now - chrono::Duration::seconds(1);
        let future = now + chrono::Duration::seconds(60);

        let id_a = NotificationAttemptId::new();
        let id_b = NotificationAttemptId::new();
        let id_c = NotificationAttemptId::new();
        let incident = IncidentId::new();
        for id in [&id_a, &id_b, &id_c] {
            repo.insert_pending(&pending(id.clone(), incident.clone(), now))
                .await
                .unwrap();
        }
        // V0.1-shape: scheduler completes with next_retry_at.
        repo.complete(&id_a, transient_receipt(now), Some(due_later))
            .await
            .unwrap();
        repo.complete(&id_b, transient_receipt(now), Some(due_early))
            .await
            .unwrap();
        repo.complete(&id_c, transient_receipt(now), Some(future))
            .await
            .unwrap();

        let due_now = repo.list_retryable(now, 10).await.unwrap();
        assert_eq!(due_now.len(), 2, "only the two due rows match");
        // Oldest-first by next_retry_at.
        assert_eq!(due_now[0].id, id_b);
        assert_eq!(due_now[1].id, id_a);
    }

    #[tokio::test]
    async fn list_retryable_query_uses_status_next_retry_index() {
        let (repo, _dir) = fresh_repo().await;
        let now = Utc.timestamp_nanos(1_700_000_000_000_000_000);

        let plan_rows = sqlx::query(
            "EXPLAIN QUERY PLAN \
             SELECT id FROM notification_attempts \
             WHERE status = ? AND next_retry_at IS NOT NULL AND next_retry_at <= ? \
             ORDER BY next_retry_at ASC \
             LIMIT ?",
        )
        .bind(status_str(&NotificationDeliveryStatus::FailedTransient))
        .bind(nanos(now))
        .bind(10_i64)
        .fetch_all(&repo.pool)
        .await
        .expect("explain query plan");

        let plan: String = plan_rows
            .iter()
            .map(|r| r.try_get::<String, _>("detail").unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            plan.contains("idx_attempts_status_next_retry"),
            "list_retryable must hit the (status, next_retry_at) index; \
             EXPLAIN QUERY PLAN was:\n{plan}"
        );
    }

    #[tokio::test]
    async fn round_trip_every_delivery_outcome_variant() {
        let (repo, _dir) = fresh_repo().await;
        let at = Utc.timestamp_nanos(1_700_000_000_000_000_000);
        let incident = IncidentId::new();

        let receipts = vec![
            delivered_receipt(at),
            delivered_discord(at),
            transient_receipt(at),
            permanent_receipt(at),
        ];

        for receipt in receipts {
            let id = NotificationAttemptId::new();
            repo.insert_pending(&pending(id.clone(), incident.clone(), at))
                .await
                .unwrap();
            repo.complete(&id, receipt.clone(), None).await.unwrap();

            let listed = repo.list_for_incident(&incident).await.unwrap();
            let got = listed.iter().find(|a| a.id == id).expect("row exists");
            assert_eq!(got.outcome.as_ref().unwrap(), &receipt.outcome);
            // External ref round-trips for Delivered, absent for everything else.
            match &receipt.outcome {
                DeliveryOutcome::Delivered { external_ref } => {
                    assert_eq!(got.external_ref.as_ref(), external_ref.as_ref());
                }
                _ => assert!(got.external_ref.is_none()),
            }
        }
    }

    #[tokio::test]
    async fn list_for_incident_orders_newest_first() {
        let (repo, _dir) = fresh_repo().await;
        let t0 = Utc.timestamp_nanos(1_700_000_000_000_000_000);
        let t1 = t0 + chrono::Duration::seconds(1);
        let t2 = t0 + chrono::Duration::seconds(2);

        let incident = IncidentId::new();
        let id0 = NotificationAttemptId::new();
        let id1 = NotificationAttemptId::new();
        let id2 = NotificationAttemptId::new();

        // Insert in reverse-chronological order to ensure ordering is
        // by attempted_at and not by insertion order.
        repo.insert_pending(&pending(id1.clone(), incident.clone(), t1))
            .await
            .unwrap();
        repo.insert_pending(&pending(id0.clone(), incident.clone(), t0))
            .await
            .unwrap();
        repo.insert_pending(&pending(id2.clone(), incident.clone(), t2))
            .await
            .unwrap();

        let listed = repo.list_for_incident(&incident).await.unwrap();
        assert_eq!(listed.len(), 3);
        assert_eq!(listed[0].id, id2);
        assert_eq!(listed[1].id, id1);
        assert_eq!(listed[2].id, id0);
    }

    #[tokio::test]
    async fn insert_pending_rejects_duplicate_id_as_conflict() {
        let (repo, _dir) = fresh_repo().await;
        let at = Utc.timestamp_nanos(1_700_000_000_000_000_000);
        let id = NotificationAttemptId::new();
        let incident = IncidentId::new();

        repo.insert_pending(&pending(id.clone(), incident.clone(), at))
            .await
            .unwrap();
        let err = repo
            .insert_pending(&pending(id.clone(), incident, at))
            .await
            .expect_err("second insert must fail");
        assert!(matches!(err, RepoError::Conflict { .. }));
    }

    #[tokio::test]
    async fn complete_unknown_id_returns_not_found() {
        let (repo, _dir) = fresh_repo().await;
        let at = Utc.timestamp_nanos(1_700_000_000_000_000_000);
        let id = NotificationAttemptId::new();
        let err = repo
            .complete(&id, delivered_receipt(at), None)
            .await
            .expect_err("complete on unknown id must fail");
        assert!(matches!(err, RepoError::NotFound { .. }));
    }
}
