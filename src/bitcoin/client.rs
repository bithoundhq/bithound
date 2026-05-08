use async_trait::async_trait;
use corepc_types::v30::{GetBlockchainInfo, GetMempoolInfo, GetNetworkInfo, GetPeerInfo};

use crate::rpc::{RpcClient, RpcError};

#[async_trait]
pub trait BitcoinClient: Send + Sync + std::fmt::Debug {
    async fn get_blockchain_info(&self) -> Result<GetBlockchainInfo, RpcError>;
    async fn get_network_info(&self) -> Result<GetNetworkInfo, RpcError>;
    async fn get_peer_info(&self) -> Result<GetPeerInfo, RpcError>;
    async fn get_mempool_info(&self) -> Result<GetMempoolInfo, RpcError>;
}

#[async_trait]
impl BitcoinClient for RpcClient {
    async fn get_blockchain_info(&self) -> Result<GetBlockchainInfo, RpcError> {
        let res = match self.call("getblockchaininfo", None).await?.result {
            Some(res) => res,
            None => return Err(RpcError::ResultUnavailable),
        };

        let info: GetBlockchainInfo = serde_json::from_value(res)?;
        Ok(info)
    }

    async fn get_network_info(&self) -> Result<GetNetworkInfo, RpcError> {
        let res = match self.call("getnetworkinfo", None).await?.result {
            Some(res) => res,
            None => return Err(RpcError::ResultUnavailable),
        };

        let info: GetNetworkInfo = serde_json::from_value(res)?;
        Ok(info)
    }

    async fn get_peer_info(&self) -> Result<GetPeerInfo, RpcError> {
        let res = match self.call("getpeerinfo", None).await?.result {
            Some(res) => res,
            None => return Err(RpcError::ResultUnavailable),
        };

        let info: GetPeerInfo = serde_json::from_value(res)?;
        Ok(info)
    }

    async fn get_mempool_info(&self) -> Result<GetMempoolInfo, RpcError> {
        let res = match self.call("getmempoolinfo", None).await?.result {
            Some(res) => res,
            None => return Err(RpcError::ResultUnavailable),
        };

        let info: GetMempoolInfo = serde_json::from_value(res)?;
        Ok(info)
    }
}
