//! Error types for the HTTP API task.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use thiserror::Error;

/// Errors surfaced by the API server task itself (bind / serve I/O).
/// Returned by [`super::server::run`].
#[derive(Debug, Error)]
pub enum ServerError {
    #[error("api bind on {addr}: {source}")]
    Bind {
        addr: std::net::SocketAddr,
        #[source]
        source: std::io::Error,
    },

    #[error("api serve: {0}")]
    Serve(#[source] std::io::Error),
}

/// Handler-level error. Implements `IntoResponse` so every handler
/// can `?` propagate through and the framework renders a JSON error
/// body with the correct status code.
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("storage error: {0}")]
    Storage(#[from] crate::incidents::repository::RepoError),

    #[error("observation store error: {0}")]
    ObservationStore(#[from] crate::storage::traits::StoreError),

    #[error("serialization: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Debug, Serialize)]
struct ApiErrorBody<'a> {
    error: &'a str,
    detail: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, tag) = match &self {
            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            ApiError::Storage(_) | ApiError::ObservationStore(_) | ApiError::Internal(_) => {
                tracing::error!(error = %self, "api handler internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
            }
            ApiError::Serialization(_) => {
                tracing::error!(error = %self, "api response serialization error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
            }
        };
        let body = ApiErrorBody {
            error: tag,
            detail: self.to_string(),
        };
        (status, Json(body)).into_response()
    }
}
