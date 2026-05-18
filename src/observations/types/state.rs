//! Types for state observations.
use serde::{Deserialize, Serialize};

use crate::observations::types::NumericValue;

pub mod well_known;

/// Structured snapshots of a subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateObservation {
    BitcoinBlockchain(BitcoinBlockchainState),
    BitcoinMempool(BitcoinMempoolState),
    BitcoinNetwork(BitcoinNetworkState),
    BitcoinPeerSummary(BitcoinPeerSummaryState),

    LndNode(LndNodeState),
    LndWallet(LndWalletState),
    LndChannelSummary(LndChannelSummaryState),

    Host(HostState),
}

impl StateObservation {
    /// Canonical name for this variant. Used by the read-model store
    /// (per ADR-R1 §R1.2) to key the generic `latest_state(subject, name)`
    /// query, and by collectors that emit state observations.
    ///
    /// The arms here MUST stay in sync with `well_known::*`. A unit test
    /// asserts parity; adding a variant without updating both will fail
    /// the build.
    pub fn name(&self) -> StateName {
        StateName(
            match self {
                Self::BitcoinBlockchain(_) => well_known::BITCOIN_BLOCKCHAIN,
                Self::BitcoinMempool(_) => well_known::BITCOIN_MEMPOOL,
                Self::BitcoinNetwork(_) => well_known::BITCOIN_NETWORK,
                Self::BitcoinPeerSummary(_) => well_known::BITCOIN_PEER_SUMMARY,
                Self::LndNode(_) => well_known::LND_NODE,
                Self::LndWallet(_) => well_known::LND_WALLET,
                Self::LndChannelSummary(_) => well_known::LND_CHANNEL_SUMMARY,
                Self::Host(_) => well_known::HOST_SYSTEM,
            }
            .to_string(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StateName(pub String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StateValue {
    Bool(bool),
    String(String),
    Number(NumericValue),
    Object(serde_json::Value),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitcoinBlockchainState {
    pub chain: String,
    pub blocks: u64,
    pub headers: u64,
    pub best_block_hash: Option<String>,
    pub verification_progress: f64,
    pub initial_block_download: bool,
    pub pruned: bool,
    pub size_on_disk_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitcoinMempoolState {
    pub loaded: bool,
    pub tx_count: u64,
    pub bytes: u64,
    pub usage_bytes: u64,
    pub max_mempool_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitcoinNetworkState {
    pub version: u64,
    pub subversion: String,
    pub protocol_version: u64,
    pub connections: u64,
    pub connections_in: Option<u64>,
    pub connections_out: Option<u64>,
    pub network_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitcoinPeerSummaryState {
    pub peer_count: u64,
    pub inbound_count: Option<u64>,
    pub outbound_count: Option<u64>,
    pub block_relay_only_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LndNodeState {
    pub identity_pubkey: String,
    pub alias: Option<String>,
    pub version: Option<String>,
    pub num_active_channels: u64,
    pub num_inactive_channels: Option<u64>,
    pub num_pending_channels: u64,
    pub num_peers: u64,
    pub block_height: u64,
    pub synced_to_chain: bool,
    pub synced_to_graph: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LndWalletState {
    pub total_balance_sat: u64,
    pub confirmed_balance_sat: u64,
    pub unconfirmed_balance_sat: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LndChannelSummaryState {
    pub active_channels: u64,
    pub inactive_channels: u64,
    pub pending_channels: u64,
    pub total_capacity_sat: Option<u64>,
    pub local_balance_sat: u64,
    pub remote_balance_sat: u64,
    pub unsettled_balance_sat: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostState {
    pub hostname: Option<String>,
    pub os: Option<String>,
    pub kernel: Option<String>,
    pub uptime_seconds: Option<u64>,
    pub cpu_count: Option<u64>,
    pub memory_total_bytes: Option<u64>,
    pub disk_total_bytes: Option<u64>,
    pub disk_available_bytes: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// One minimal sample of each `StateObservation` variant.
    /// Adding a variant without updating this list (and `well_known::ALL`)
    /// will cause `parity_variants_match_well_known` to fail.
    fn one_of_each_variant() -> Vec<StateObservation> {
        vec![
            StateObservation::BitcoinBlockchain(BitcoinBlockchainState {
                chain: "main".into(),
                blocks: 0,
                headers: 0,
                best_block_hash: None,
                verification_progress: 1.0,
                initial_block_download: false,
                pruned: false,
                size_on_disk_bytes: 0,
            }),
            StateObservation::BitcoinMempool(BitcoinMempoolState {
                loaded: true,
                tx_count: 0,
                bytes: 0,
                usage_bytes: 0,
                max_mempool_bytes: 0,
            }),
            StateObservation::BitcoinNetwork(BitcoinNetworkState {
                version: 0,
                subversion: String::new(),
                protocol_version: 0,
                connections: 0,
                connections_in: None,
                connections_out: None,
                network_active: None,
            }),
            StateObservation::BitcoinPeerSummary(BitcoinPeerSummaryState {
                peer_count: 0,
                inbound_count: None,
                outbound_count: None,
                block_relay_only_count: None,
            }),
            StateObservation::LndNode(LndNodeState {
                identity_pubkey: String::new(),
                alias: None,
                version: None,
                num_active_channels: 0,
                num_inactive_channels: None,
                num_pending_channels: 0,
                num_peers: 0,
                block_height: 0,
                synced_to_chain: false,
                synced_to_graph: None,
            }),
            StateObservation::LndWallet(LndWalletState {
                total_balance_sat: 0,
                confirmed_balance_sat: 0,
                unconfirmed_balance_sat: 0,
            }),
            StateObservation::LndChannelSummary(LndChannelSummaryState {
                active_channels: 0,
                inactive_channels: 0,
                pending_channels: 0,
                total_capacity_sat: None,
                local_balance_sat: 0,
                remote_balance_sat: 0,
                unsettled_balance_sat: None,
            }),
            StateObservation::Host(HostState {
                hostname: None,
                os: None,
                kernel: None,
                uptime_seconds: None,
                cpu_count: None,
                memory_total_bytes: None,
                disk_total_bytes: None,
                disk_available_bytes: None,
            }),
        ]
    }

    #[test]
    fn each_variant_name_is_a_well_known_constant() {
        let well_known: HashSet<&'static str> = well_known::ALL.iter().copied().collect();
        for variant in one_of_each_variant() {
            let name = variant.name().0;
            assert!(
                well_known.contains(name.as_str()),
                "variant {:?} produced name {name:?} not in well_known::ALL",
                std::mem::discriminant(&variant),
            );
        }
    }

    #[test]
    fn parity_variants_match_well_known() {
        let from_variants: HashSet<String> = one_of_each_variant()
            .into_iter()
            .map(|v| v.name().0)
            .collect();
        let from_constants: HashSet<String> =
            well_known::ALL.iter().map(|s| s.to_string()).collect();
        assert_eq!(
            from_variants, from_constants,
            "StateObservation variants and well_known::ALL must be in 1:1 correspondence"
        );
    }

    #[test]
    fn name_returns_expected_string_per_variant() {
        let cases: [(StateObservation, &str); 8] = [
            (
                StateObservation::BitcoinBlockchain(BitcoinBlockchainState {
                    chain: "main".into(),
                    blocks: 0,
                    headers: 0,
                    best_block_hash: None,
                    verification_progress: 0.0,
                    initial_block_download: false,
                    pruned: false,
                    size_on_disk_bytes: 0,
                }),
                well_known::BITCOIN_BLOCKCHAIN,
            ),
            (
                StateObservation::BitcoinMempool(BitcoinMempoolState {
                    loaded: true,
                    tx_count: 0,
                    bytes: 0,
                    usage_bytes: 0,
                    max_mempool_bytes: 0,
                }),
                well_known::BITCOIN_MEMPOOL,
            ),
            (
                StateObservation::BitcoinNetwork(BitcoinNetworkState {
                    version: 0,
                    subversion: String::new(),
                    protocol_version: 0,
                    connections: 0,
                    connections_in: None,
                    connections_out: None,
                    network_active: None,
                }),
                well_known::BITCOIN_NETWORK,
            ),
            (
                StateObservation::BitcoinPeerSummary(BitcoinPeerSummaryState {
                    peer_count: 0,
                    inbound_count: None,
                    outbound_count: None,
                    block_relay_only_count: None,
                }),
                well_known::BITCOIN_PEER_SUMMARY,
            ),
            (
                StateObservation::LndNode(LndNodeState {
                    identity_pubkey: String::new(),
                    alias: None,
                    version: None,
                    num_active_channels: 0,
                    num_inactive_channels: None,
                    num_pending_channels: 0,
                    num_peers: 0,
                    block_height: 0,
                    synced_to_chain: false,
                    synced_to_graph: None,
                }),
                well_known::LND_NODE,
            ),
            (
                StateObservation::LndWallet(LndWalletState {
                    total_balance_sat: 0,
                    confirmed_balance_sat: 0,
                    unconfirmed_balance_sat: 0,
                }),
                well_known::LND_WALLET,
            ),
            (
                StateObservation::LndChannelSummary(LndChannelSummaryState {
                    active_channels: 0,
                    inactive_channels: 0,
                    pending_channels: 0,
                    total_capacity_sat: None,
                    local_balance_sat: 0,
                    remote_balance_sat: 0,
                    unsettled_balance_sat: None,
                }),
                well_known::LND_CHANNEL_SUMMARY,
            ),
            (
                StateObservation::Host(HostState {
                    hostname: None,
                    os: None,
                    kernel: None,
                    uptime_seconds: None,
                    cpu_count: None,
                    memory_total_bytes: None,
                    disk_total_bytes: None,
                    disk_available_bytes: None,
                }),
                well_known::HOST_SYSTEM,
            ),
        ];
        for (variant, expected) in cases {
            assert_eq!(variant.name().0, expected);
        }
    }
}
