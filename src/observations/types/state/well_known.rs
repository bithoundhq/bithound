//! Canonical `StateName` string constants, one per `StateObservation` variant.
//!
//! The values here are the strings rules pass to
//! [`crate::read_models::traits::state::StateReadModel::latest_state`] when
//! they want the latest state of a given kind. A parity unit test in
//! `state.rs` asserts that every variant of [`super::StateObservation`]
//! has a matching constant and vice versa.

pub const BITCOIN_BLOCKCHAIN: &str = "bitcoin.blockchain";
pub const BITCOIN_MEMPOOL: &str = "bitcoin.mempool";
pub const BITCOIN_NETWORK: &str = "bitcoin.network";
pub const BITCOIN_PEER_SUMMARY: &str = "bitcoin.peer_summary";
pub const LND_NODE: &str = "lnd.node";
pub const LND_WALLET: &str = "lnd.wallet";
pub const LND_CHANNEL_SUMMARY: &str = "lnd.channel_summary";
/// Per-instance channel state (each observation describes one channel).
/// Distinguished from `LND_CHANNEL_SUMMARY` which is aggregate counts.
pub const LND_CHANNEL_DETAIL: &str = "lnd.channel_detail";
pub const HOST_SYSTEM: &str = "host.system";

/// Every canonical state name, used by parity tests and by future
/// validation passes that need to enumerate known kinds.
pub const ALL: &[&str] = &[
    BITCOIN_BLOCKCHAIN,
    BITCOIN_MEMPOOL,
    BITCOIN_NETWORK,
    BITCOIN_PEER_SUMMARY,
    LND_NODE,
    LND_WALLET,
    LND_CHANNEL_SUMMARY,
    LND_CHANNEL_DETAIL,
    HOST_SYSTEM,
];
