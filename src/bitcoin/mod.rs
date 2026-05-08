//! Bitcoin Core telemetry domain.
//!
//! [`BitcoinTelemetry`] orchestrates periodic collection from a
//! [`BitcoinClient`] (typically the crate's `RpcClient`) and exposes the
//! aggregate state as a [`BitcoinSnapshot`].

mod client;
mod telemetry;
mod types;

use crate::rpc::RpcError;

/// Errors produced while interacting with a Bitcoin Core backend.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Rpc(#[from] RpcError),
    #[error("bitcoind returned empty response")]
    EmptyResponse,
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
