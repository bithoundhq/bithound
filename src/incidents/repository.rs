//! `IncidentRepository` trait per ADR-L4 §L4.6.
//!
//! Concrete SQLite impl lives in `crate::storage::sqlite::incident_repository`;
//! the in-memory test impl lives under `crate::storage::memory`.

use async_trait::async_trait;

use crate::incidents::Incident;
use crate::shared::types::IncidentId;

#[async_trait]
pub trait IncidentRepository: Send + Sync {
    /// Load every incident whose status is not `Resolved`.
    ///
    /// Used at startup by the runtime to hydrate the engine's
    /// open-incident map.
    async fn load_open(&self) -> Result<Vec<Incident>, RepoError>;

    /// Upsert the incident — INSERT on new id, UPDATE on existing id, all
    /// columns replaced atomically.
    async fn save(&self, incident: &Incident) -> Result<(), RepoError>;

    /// Fetch a single incident by id, regardless of status. Returns
    /// `None` if no row matches (either it never existed or retention
    /// swept it). Used by the operator API's `/incidents/:id` endpoint.
    async fn get(&self, id: &IncidentId) -> Result<Option<Incident>, RepoError>;
}

#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    #[error("backend: {0}")]
    Backend(String),

    #[error("conflict on incident {id:?}")]
    Conflict { id: IncidentId },

    #[error("incident not found: {id:?}")]
    NotFound { id: IncidentId },
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
