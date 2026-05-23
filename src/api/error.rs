//! Error types for the HTTP API task.

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
