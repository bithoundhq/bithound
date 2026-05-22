//! Bitcoin Core integration — RPC client and the polling collector
//! built on top of it.

pub mod rpc;
pub mod rpc_client;

pub use rpc::{BitcoinCoreRpcCollector, BitcoinCoreRpcCollectorConfig, BuildError};
pub use rpc_client::{
    BitcoinRpcClient, GetBlockchainInfoResponse, GetMempoolInfoResponse, GetNetworkInfoResponse,
    GetPeerInfoResponse, PeerInfoEntry, RpcError,
};
