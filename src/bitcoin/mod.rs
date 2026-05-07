use crate::rpc::RpcError;

mod client;
mod telemetry;
mod types;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Rpc(#[from] RpcError),
    #[error("bitcoind returned empty response")]
    EmptyResponse,
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

