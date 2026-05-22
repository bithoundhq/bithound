//! `NotificationAttemptRepository` trait.
//!
//! SQLite impl lives in `crate::storage::sqlite::notification_attempt_repository`
//! (BTH-52); the in-memory test impl lives under `crate::storage::memory`.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::notifications::types::{DeliveryReceipt, NotificationAttempt, NotificationAttemptId};
use crate::shared::types::IncidentId;

#[async_trait]
pub trait NotificationAttemptRepository: Send + Sync {
    /// INSERT a row with `status = Pending`. Called before dispatch.
    async fn insert_pending(&self, attempt: &NotificationAttempt) -> Result<(), RepoError>;

    /// UPDATE an existing row from `Pending` to a terminal status.
    ///
    /// `next_retry_at` is `Some(t)` iff the outcome is `Transient` and
    /// retries remain (V0.1+). V0 always passes `None`.
    async fn complete(
        &self,
        id: &NotificationAttemptId,
        receipt: DeliveryReceipt,
        next_retry_at: Option<DateTime<Utc>>,
    ) -> Result<(), RepoError>;

    /// Rows in `FailedTransient` with `next_retry_at <= now`, oldest first.
    ///
    /// Used by the V0.1+ retry scheduler. In V0 the result is always empty
    /// (no attempt ever lands in `FailedTransient`).
    async fn list_retryable(
        &self,
        now: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<NotificationAttempt>, RepoError>;

    /// All attempts for an incident, newest first. Operator-UI scaffolding
    /// (V0.2 surface, available now for audit).
    async fn list_for_incident(
        &self,
        incident_id: &IncidentId,
    ) -> Result<Vec<NotificationAttempt>, RepoError>;
}

#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    #[error("backend: {0}")]
    Backend(String),

    #[error("conflict on attempt {id:?}")]
    Conflict { id: NotificationAttemptId },

    #[error("attempt not found: {id:?}")]
    NotFound { id: NotificationAttemptId },
}

impl From<sqlx::Error> for RepoError {
    fn from(err: sqlx::Error) -> Self {
        RepoError::Backend(err.to_string())
    }
}

impl From<serde_json::Error> for RepoError {
    fn from(err: serde_json::Error) -> Self {
        RepoError::Backend(format!("serde_json: {err}"))
    }
}
