//! Local operator HTTP API.
//!
//! Scaffolding pass: binds loopback-only by default, runs as a third
//! tokio task alongside the consumer and notification worker, and
//! gracefully exits on the shutdown broadcast. No routes are mounted
//! yet — the four V0 operator endpoints land in the follow-up
//! handlers commit.

pub mod error;
pub mod server;

use std::sync::Arc;

use axum::Router;

use crate::incidents::repository::IncidentRepository;
use crate::notifications::repository::NotificationAttemptRepository;
use crate::shared::types::SidecarId;
use crate::storage::traits::ObservationStore;

/// Repository handles the API task needs. The API is read-only —
/// every handle is an `Arc<dyn ...>` and the task never mutates state.
#[derive(Clone)]
pub struct ApiDeps {
    pub sidecar_id: SidecarId,
    pub sidecar_version: &'static str,
    pub started_at: std::time::Instant,
    pub incident_repo: Arc<dyn IncidentRepository>,
    pub observation_store: Arc<dyn ObservationStore>,
    pub attempts_repo: Arc<dyn NotificationAttemptRepository>,
}

/// Build the axum router. No routes mounted in the scaffolding pass.
pub fn build_router(_deps: ApiDeps) -> Router {
    Router::new()
}
