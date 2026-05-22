//! Diagnostic rules for Bitcoin Core nodes.

pub mod no_peers;
pub mod rpc_unreachable;
pub mod tip_lag_or_ibd_stalled;

#[cfg(test)]
pub(crate) mod test_support;

pub use no_peers::BitcoinNoPeersRule;
pub use rpc_unreachable::BitcoinRpcUnreachableRule;
pub use tip_lag_or_ibd_stalled::BitcoinTipLagOrIbdStalledRule;
