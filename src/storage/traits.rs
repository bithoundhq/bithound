//! Storage trait surfaces.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::stream::BoxStream;

use crate::observations::Observation;

/// Append-only store for [`Observation`] facts.
///
/// V0 ships the full surface (append + iter). The trait is kept minimal —
/// single mutation method (`append_many`) plus a streaming read
/// (`iter_since`) used by V0.1+ read-model replay.
///
/// Concrete impls (`SqliteObservationStore`, `MemoryObservationStore`) live
/// under `storage::sqlite` and `storage::memory`.
#[async_trait]
pub trait ObservationStore: Send + Sync {
    /// Persist a batch of observations atomically.
    async fn append_many(&self, batch: &[Observation]) -> Result<(), StoreError>;

    /// Stream observations whose `observed_at >= since`, in ascending order.
    async fn iter_since(
        &self,
        since: DateTime<Utc>,
    ) -> Result<BoxStream<'_, Result<Observation, StoreError>>, StoreError>;

    /// Single-observation convenience that delegates to [`Self::append_many`].
    async fn append(&self, obs: &Observation) -> Result<(), StoreError> {
        self.append_many(std::slice::from_ref(obs)).await
    }
}

/// Errors produced by [`ObservationStore`] (and shared with the SQLite-backed
/// `IncidentRepository` / `NotificationAttemptRepository` impls for I/O,
/// database, and serialization failures).
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("database: {0}")]
    Database(#[from] sqlx::Error),

    #[error("serialization: {0}")]
    Serialization(#[from] serde_json::Error),

    /// A stored row failed to deserialize into its domain type.
    ///
    /// Logged and skipped at the call site; the database itself is treated
    /// as authoritative.
    #[error("corruption: {0}")]
    Corruption(String),

    /// The store was used before `open` (or equivalent) had run.
    #[error("not initialized")]
    NotInitialized,
}
