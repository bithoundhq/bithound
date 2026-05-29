//! Diagnostic rules for LND nodes and their channels.

pub mod chain_backend_lag;
pub mod channel_inactive;

pub use chain_backend_lag::LndChainBackendLagRule;
pub use channel_inactive::LndChannelInactiveRule;
