//! Local operator HTTP API.
//!
//! V0 surface is four read-only endpoints (`GET /health`,
//! `GET /incidents/open`, `GET /incidents/:id`,
//! `GET /incidents/:id/evidence`). Binds loopback-only by default;
//! no auth, no CORS, no TLS — the loopback bind is the safety
//! mechanism.

pub mod dto;
pub mod error;
pub mod handlers;
pub mod server;

use std::sync::Arc;

use axum::routing::get;
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

/// Build the axum router with the V0 operator endpoints mounted.
pub fn build_router(deps: ApiDeps) -> Router {
    Router::new()
        .route("/health", get(handlers::health::handler))
        .route("/incidents/open", get(handlers::incidents::list_open))
        .route("/incidents/:id", get(handlers::incidents::detail))
        .route(
            "/incidents/:id/evidence",
            get(handlers::incidents::evidence),
        )
        .with_state(deps)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::dto::{HealthDto, IncidentDetailDto, IncidentEvidenceDto, IncidentListDto};
    use crate::incidents::{
        Incident, IncidentFingerprint, IncidentKind, IncidentSeverity, IncidentStatus,
    };
    use crate::observations::{
        Attributes, EventName, EventSeverity, Observation, ObservationContext, ObservationOrigin,
        ObservationSource,
    };
    use crate::shared::types::{
        BitcoinNodeId, CollectorId, EntityRef, EvidenceRef, IncidentId, ObservationId, SidecarId,
    };
    use crate::storage::memory::incident_repository::MemoryIncidentRepository;
    use crate::storage::memory::notification_attempt_repository::MemoryNotificationAttemptRepository;
    use crate::storage::memory::observation_store::MemoryObservationStore;
    use crate::storage::traits::ObservationStore;
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use chrono::{TimeZone, Utc};
    use std::collections::BTreeMap;
    use tower::ServiceExt;
    use uuid::Uuid;

    fn t0() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 23, 12, 0, 0).unwrap()
    }

    fn ctx(subject: EntityRef) -> ObservationContext {
        ObservationContext {
            source: ObservationSource {
                sidecar_id: SidecarId(Uuid::now_v7()),
                collector: crate::collectors::CollectorRef {
                    id: CollectorId("alice-rpc".into()),
                    integration: crate::collectors::IntegrationKind::BitcoinCoreRpc {
                        interval: chrono::Duration::seconds(10),
                    },
                    instance_label: "alice".into(),
                },
            },
            subject,
            observed_at: t0(),
            origin: ObservationOrigin::Collected,
        }
    }

    fn evidence_obs() -> Observation {
        Observation::event(
            ctx(EntityRef::BitcoinNode(BitcoinNodeId("alice".into()))),
            EventName::parse("bitcoin.test").expect("valid"),
            EventSeverity::Info,
            None,
            Attributes(BTreeMap::new()),
        )
    }

    fn sample_incident(id: IncidentId, evidence_ids: Vec<ObservationId>) -> Incident {
        let subject = EntityRef::BitcoinNode(BitcoinNodeId("alice".into()));
        let kind = IncidentKind::parse("bitcoin.no_peers").expect("valid");
        Incident {
            id,
            fingerprint: IncidentFingerprint {
                subject: subject.clone(),
                kind: kind.clone(),
                dimension: None,
            },
            kind,
            subject,
            severity: IncidentSeverity::Critical,
            status: IncidentStatus::Open,
            opened_at: t0(),
            updated_at: t0(),
            resolved_at: None,
            signal_observation_ids: vec![],
            evidence: evidence_ids.into_iter().map(EvidenceRef).collect(),
            summary: "no peers".into(),
            evidence_summary: vec!["3 minutes without outbound peers".into()],
        }
    }

    async fn build_deps_with_one_incident_and_evidence() -> (ApiDeps, IncidentId, ObservationId) {
        let incident_repo = std::sync::Arc::new(MemoryIncidentRepository::new());
        let observation_store = std::sync::Arc::new(MemoryObservationStore::new());
        let attempts_repo = std::sync::Arc::new(MemoryNotificationAttemptRepository::new());

        let obs = evidence_obs();
        let obs_id = obs.id.clone();
        observation_store.append(&obs).await.unwrap();

        let incident_id = IncidentId::new();
        let incident = sample_incident(incident_id.clone(), vec![obs_id.clone()]);
        incident_repo.save(&incident).await.unwrap();

        let deps = ApiDeps {
            sidecar_id: SidecarId(Uuid::now_v7()),
            sidecar_version: "test-0.0.0",
            started_at: std::time::Instant::now(),
            incident_repo,
            observation_store,
            attempts_repo,
        };
        (deps, incident_id, obs_id)
    }

    async fn body_json<T: serde::de::DeserializeOwned>(body: Body) -> T {
        let bytes = to_bytes(body, 1_048_576).await.expect("read body");
        serde_json::from_slice(&bytes).expect("parse json")
    }

    /// `/health` returns 200 + the documented JSON shape when the DB
    /// is reachable.
    #[tokio::test]
    async fn health_returns_200_with_db_reachable() {
        let (deps, _, _) = build_deps_with_one_incident_and_evidence().await;
        let app = build_router(deps);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let dto: HealthDto = body_json(resp.into_body()).await;
        assert_eq!(dto.version, "test-0.0.0");
        assert!(dto.db.reachable);
    }

    /// `/incidents/open` returns 200 with an empty list when no
    /// incidents are present.
    #[tokio::test]
    async fn incidents_open_empty_returns_zero_count() {
        let incident_repo = std::sync::Arc::new(MemoryIncidentRepository::new());
        let observation_store = std::sync::Arc::new(MemoryObservationStore::new());
        let attempts_repo = std::sync::Arc::new(MemoryNotificationAttemptRepository::new());
        let deps = ApiDeps {
            sidecar_id: SidecarId(Uuid::now_v7()),
            sidecar_version: "test-0.0.0",
            started_at: std::time::Instant::now(),
            incident_repo,
            observation_store,
            attempts_repo,
        };

        let app = build_router(deps);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/incidents/open")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let dto: IncidentListDto = body_json(resp.into_body()).await;
        assert_eq!(dto.count, 0);
        assert!(dto.incidents.is_empty());
    }

    /// `/incidents/open` returns the summary for a saved incident.
    #[tokio::test]
    async fn incidents_open_returns_saved_incident() {
        let (deps, id, _) = build_deps_with_one_incident_and_evidence().await;
        let app = build_router(deps);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/incidents/open")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let dto: IncidentListDto = body_json(resp.into_body()).await;
        assert_eq!(dto.count, 1);
        assert_eq!(dto.incidents[0].id, id.0);
        assert_eq!(dto.incidents[0].kind, "bitcoin.no_peers");
    }

    /// `/incidents/:id` round-trips the full detail DTO.
    #[tokio::test]
    async fn incidents_detail_returns_full_incident() {
        let (deps, id, _) = build_deps_with_one_incident_and_evidence().await;
        let app = build_router(deps);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/incidents/{id_str}", id_str = id.0))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let dto: IncidentDetailDto = body_json(resp.into_body()).await;
        assert_eq!(dto.id, id.0);
        assert_eq!(dto.summary, "no peers");
        assert_eq!(dto.evidence_summary.len(), 1);
    }

    /// `/incidents/:id` returns 404 for an unknown id.
    #[tokio::test]
    async fn incidents_detail_404_on_unknown_id() {
        let (deps, _, _) = build_deps_with_one_incident_and_evidence().await;
        let app = build_router(deps);
        let unknown = Uuid::now_v7();
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/incidents/{unknown}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    /// `/incidents/:id` returns 400 for a non-UUID path.
    #[tokio::test]
    async fn incidents_detail_400_on_bad_id() {
        let (deps, _, _) = build_deps_with_one_incident_and_evidence().await;
        let app = build_router(deps);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/incidents/not-a-uuid")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    /// `/incidents/:id/evidence` returns the dereferenced observations.
    #[tokio::test]
    async fn incidents_evidence_returns_referenced_observations() {
        let (deps, incident_id, obs_id) = build_deps_with_one_incident_and_evidence().await;
        let app = build_router(deps);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/incidents/{}/evidence", incident_id.0))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let dto: IncidentEvidenceDto = body_json(resp.into_body()).await;
        assert_eq!(dto.incident_id, incident_id.0);
        assert_eq!(dto.evidence.len(), 1);
        assert_eq!(dto.evidence[0].observation_id, obs_id.0);
    }

    /// Evidence observations swept by retention are silently omitted —
    /// the incident references the id but the observation store
    /// returns `None` for it.
    #[tokio::test]
    async fn incidents_evidence_silently_omits_swept_observations() {
        let incident_repo = std::sync::Arc::new(MemoryIncidentRepository::new());
        let observation_store = std::sync::Arc::new(MemoryObservationStore::new());
        let attempts_repo = std::sync::Arc::new(MemoryNotificationAttemptRepository::new());

        // Reference an observation id that was never appended to the store.
        let phantom = ObservationId::new();
        let incident_id = IncidentId::new();
        let incident = sample_incident(incident_id.clone(), vec![phantom]);
        incident_repo.save(&incident).await.unwrap();

        let deps = ApiDeps {
            sidecar_id: SidecarId(Uuid::now_v7()),
            sidecar_version: "test-0.0.0",
            started_at: std::time::Instant::now(),
            incident_repo,
            observation_store,
            attempts_repo,
        };
        let app = build_router(deps);
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/incidents/{}/evidence", incident_id.0))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let dto: IncidentEvidenceDto = body_json(resp.into_body()).await;
        assert!(
            dto.evidence.is_empty(),
            "swept observations must be silently omitted, got: {dto:?}"
        );
    }
}
