//! `GET /health` — sidecar liveness + DB connectivity.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use crate::api::dto::{DbHealthDto, HealthDto};
use crate::api::ApiDeps;

/// The handler measures a coarse "is the DB reachable" by probing
/// `incident_repo.load_open` — any storage trait call will route
/// through the SQLite pool in production, exercising the same path
/// real traffic would hit. We don't surface the loaded incident list
/// from this endpoint (that's `/incidents/open`) — only the latency
/// and reachability.
pub async fn handler(State(deps): State<ApiDeps>) -> impl IntoResponse {
    let started = std::time::Instant::now();
    let db_result = deps.incident_repo.load_open().await;
    let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let (status, db) = match &db_result {
        Ok(_) => (
            StatusCode::OK,
            DbHealthDto {
                reachable: true,
                latency_ms: Some(latency_ms),
            },
        ),
        Err(e) => {
            tracing::warn!(error = ?e, "health probe: db unreachable");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                DbHealthDto {
                    reachable: false,
                    latency_ms: None,
                },
            )
        }
    };

    let uptime = deps.started_at.elapsed().as_secs();
    let body = HealthDto {
        sidecar_id: deps.sidecar_id.0,
        version: deps.sidecar_version.to_string(),
        uptime_seconds: uptime,
        // V0 has no heartbeat producer yet (the field exists for
        // forward-compat with the operator-facing schema).
        latest_heartbeat_at: None,
        db,
    };
    (status, Json(body))
}
