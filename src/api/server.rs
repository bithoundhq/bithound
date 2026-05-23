//! axum HTTP server task.
//!
//! Lives as a third tokio task alongside the consumer and notification
//! worker. On shutdown broadcast, finishes in-flight requests then
//! exits via `axum::serve(...).with_graceful_shutdown(...)`.

use std::net::SocketAddr;

use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tower_http::trace::TraceLayer;

use crate::api::error::ServerError;
use crate::api::{build_router, ApiDeps};

/// Bind the listener, build the router, and run until the shutdown
/// broadcast fires. Returns `Ok(())` on graceful shutdown; returns
/// `Err(ServerError::Bind)` if the address is in use or denied; returns
/// `Err(ServerError::Serve)` for unexpected I/O errors mid-serve.
pub async fn run(
    bind: SocketAddr,
    deps: ApiDeps,
    mut shutdown: broadcast::Receiver<()>,
) -> Result<(), ServerError> {
    let listener = TcpListener::bind(bind)
        .await
        .map_err(|source| ServerError::Bind { addr: bind, source })?;

    let local_addr = listener
        .local_addr()
        .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], 0)));
    tracing::info!(addr = %local_addr, "operator api listening");

    let app = build_router(deps).layer(TraceLayer::new_for_http());

    let shutdown_future = async move {
        let _ = shutdown.recv().await;
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_future)
        .await
        .map_err(ServerError::Serve)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::incidents::repository::IncidentRepository;
    use crate::notifications::repository::NotificationAttemptRepository;
    use crate::storage::memory::incident_repository::MemoryIncidentRepository;
    use crate::storage::memory::observation_store::MemoryObservationStore;
    use crate::storage::traits::ObservationStore;
    use std::sync::Arc;
    use std::time::Duration;
    use uuid::Uuid;

    use crate::shared::types::SidecarId;

    fn deps() -> ApiDeps {
        let observation_store: Arc<dyn ObservationStore> = Arc::new(MemoryObservationStore::new());
        let incident_repo: Arc<dyn IncidentRepository> = Arc::new(MemoryIncidentRepository::new());
        let attempts_repo: Arc<dyn NotificationAttemptRepository> =
            Arc::new(crate::storage::memory::notification_attempt_repository::MemoryNotificationAttemptRepository::new());
        ApiDeps {
            sidecar_id: SidecarId(Uuid::now_v7()),
            sidecar_version: "test-0.0.0",
            started_at: std::time::Instant::now(),
            incident_repo,
            observation_store,
            attempts_repo,
        }
    }

    /// Bind on `127.0.0.1:0` (ephemeral port), confirm `GET /` returns
    /// 404 (no route mounted at `/`), then fire the broadcast and
    /// confirm the task exits within a generous deadline.
    #[tokio::test]
    async fn server_binds_serves_404_on_root_then_shuts_down() {
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        let shutdown_rx = shutdown_tx.subscribe();

        // Pick an ephemeral port by binding to :0 ourselves to learn the
        // chosen port, then close the listener and re-bind in the task.
        // A small race is acceptable in a single-threaded test runtime.
        let probe = TcpListener::bind("127.0.0.1:0").await.expect("probe bind");
        let addr = probe.local_addr().expect("local_addr");
        drop(probe);

        let handle = tokio::spawn(async move { run(addr, deps(), shutdown_rx).await });
        // Give the server a moment to actually bind.
        tokio::time::sleep(Duration::from_millis(100)).await;

        let body = reqwest::get(format!("http://{addr}/"))
            .await
            .expect("GET /");
        assert_eq!(body.status().as_u16(), 404);

        // Trigger graceful shutdown.
        let _ = shutdown_tx.send(());
        let out = tokio::time::timeout(Duration::from_secs(5), handle).await;
        assert!(out.is_ok(), "server must exit within 5s of shutdown");
    }

    #[tokio::test]
    async fn bind_error_surfaces_as_server_error_bind() {
        // Hold a listener on an ephemeral port so the API task can't
        // bind there.
        let occupied = TcpListener::bind("127.0.0.1:0").await.expect("hold port");
        let addr = occupied.local_addr().expect("local_addr");

        let (_tx, rx) = broadcast::channel::<()>(1);
        let err = run(addr, deps(), rx).await.unwrap_err();
        match err {
            ServerError::Bind { addr: a, .. } => assert_eq!(a, addr),
            other => panic!("expected ServerError::Bind, got {other:?}"),
        }
    }
}
