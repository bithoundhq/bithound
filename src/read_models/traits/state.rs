use crate::{
    observations::{
        BitcoinBlockchainState, BitcoinMempoolState, BitcoinNetworkState, BitcoinPeerSummaryState,
        LndChannelSummaryState, LndNodeState, LndWalletState,
    },
    read_models::Projected,
    shared::types::{BitcoinNodeId, LndNodeId},
};

pub trait StateReadModel: Send + Sync + std::fmt::Debug {
    fn bitcoin_blockchain(&self, node: &BitcoinNodeId)
        -> Option<Projected<BitcoinBlockchainState>>;
    fn bitcoin_mempool(&self, node: &BitcoinNodeId) -> Option<Projected<BitcoinMempoolState>>;
    fn bitcoin_network(&self, node: &BitcoinNodeId) -> Option<Projected<BitcoinNetworkState>>;
    fn bitcoin_peer_summary(
        &self,
        node: &BitcoinNodeId,
    ) -> Option<Projected<BitcoinPeerSummaryState>>;

    fn lnd_node(&self, node: &LndNodeId) -> Option<Projected<LndNodeState>>;
    fn lnd_wallet(&self, node: &LndNodeId) -> Option<Projected<LndWalletState>>;
    fn lnd_channel_summary(&self, node: &LndNodeId) -> Option<Projected<LndChannelSummaryState>>;
}
