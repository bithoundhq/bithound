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
    LndChannel(LndChannelState),

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
        StateName::from_well_known(match self {
            Self::BitcoinBlockchain(_) => well_known::BITCOIN_BLOCKCHAIN,
            Self::BitcoinMempool(_) => well_known::BITCOIN_MEMPOOL,
            Self::BitcoinNetwork(_) => well_known::BITCOIN_NETWORK,
            Self::BitcoinPeerSummary(_) => well_known::BITCOIN_PEER_SUMMARY,
            Self::LndNode(_) => well_known::LND_NODE,
            Self::LndWallet(_) => well_known::LND_WALLET,
            Self::LndChannelSummary(_) => well_known::LND_CHANNEL_SUMMARY,
            Self::LndChannel(_) => well_known::LND_CHANNEL_DETAIL,
            Self::Host(_) => well_known::HOST_SYSTEM,
        })
    }
}

/// Canonical name for a state observation (e.g. `bitcoin.blockchain`).
///
/// Constructed only through [`StateName::parse`] or
/// [`StateName::from_well_known`]; the inner field is private so
/// callers can't bypass validation by wrapping arbitrary strings (per
/// ADR-D2).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct StateName(String);

impl StateName {
    pub fn parse(s: impl AsRef<str>) -> Result<Self, crate::shared::parse::ParseDottedNameError> {
        crate::shared::parse::parse_dotted_name(s.as_ref()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Lift a `&'static str` known to satisfy the grammar. Debug-asserts
    /// the parse rule; release builds skip the check (BTH-6's parity
    /// test in this module guarantees every `well_known::*` constant
    /// satisfies the grammar).
    pub fn from_well_known(name: &'static str) -> Self {
        debug_assert!(
            crate::shared::parse::parse_dotted_name(name).is_ok(),
            "invalid well_known state name: {name}"
        );
        StateName(name.to_string())
    }
}

impl AsRef<str> for StateName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for StateName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for StateName {
    type Error = crate::shared::parse::ParseDottedNameError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl From<StateName> for String {
    fn from(n: StateName) -> String {
        n.0
    }
}

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

/// Per-channel state for an LND channel. Subject is
/// `EntityRef::LndChannel { node_id, channel_id }` where `channel_id`
/// is the funding outpoint (`"txid:vout"`) — stable across the
/// channel's whole lifecycle. The short-channel-id is informational
/// only; it changes form between pending and confirmed and therefore
/// cannot serve as identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LndChannelState {
    /// 33-byte secp256k1 pubkey of the remote node, hex-encoded.
    pub remote_pubkey: String,
    pub capacity_sat: u64,
    pub local_balance_sat: u64,
    pub remote_balance_sat: u64,
    /// Mirror of LND's `lnrpc.Channel.active` — `true` when the
    /// channel is currently usable for routing. The B1
    /// `lnd.channel_inactive` rule reads this field as-is; no
    /// re-derivation.
    pub active: bool,
    pub private: bool,
    pub initiator: bool,
    pub csv_delay: u32,
    pub commit_fee_sat: u64,
    /// Seconds since channel open.
    pub lifetime_seconds: u64,
    /// Block height at which LND last observed the channel update
    /// gossip. `None` for brand-new channels with no updates yet.
    pub last_update_height: Option<u64>,
    /// Short-channel-id once the funding tx is confirmed enough for
    /// gossip propagation (LND populates this in
    /// `lnrpc.Channel.chan_id` after ~6 confirmations). `None` for
    /// pending or unannounced channels. Informational only; channel
    /// identity uses `LndChannelId` (the funding outpoint).
    pub short_channel_id: Option<String>,
    /// Derived at poll time by the LND polling collector
    /// (ADR-E2 § E2.5) by cross-referencing the channel's
    /// `remote_pubkey` against the same poll tick's `ListPeers`
    /// response. `Some(true)`/`Some(false)` when the cross-reference
    /// succeeds; `None` when it can't be made (e.g. `ListPeers`
    /// failed while `ListChannels` succeeded, so the channel
    /// observation lands as a partial). The B1 rule uses this to
    /// distinguish "channel inactive because peer offline" (routine)
    /// from "channel inactive while peer is online" (suspicious).
    pub peer_online: Option<bool>,
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
            StateObservation::LndChannel(LndChannelState {
                remote_pubkey: String::new(),
                capacity_sat: 0,
                local_balance_sat: 0,
                remote_balance_sat: 0,
                active: true,
                private: false,
                initiator: false,
                csv_delay: 0,
                commit_fee_sat: 0,
                lifetime_seconds: 0,
                last_update_height: None,
                short_channel_id: None,
                peer_online: None,
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
            let name = variant.name();
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
            .map(|v| v.name().as_str().to_string())
            .collect();
        let from_constants: HashSet<String> =
            well_known::ALL.iter().map(|s| s.to_string()).collect();
        assert_eq!(
            from_variants, from_constants,
            "StateObservation variants and well_known::ALL must be in 1:1 correspondence"
        );
    }

    /// Every well-known state name must satisfy the shared dotted-name
    /// grammar so `StateName::from_well_known` is a safe fast path in
    /// release builds (where the debug-assert is compiled out). This
    /// is what makes [`StateObservation::name`] panic-free.
    #[test]
    fn all_well_known_state_names_parse() {
        for name in well_known::ALL {
            crate::shared::parse::parse_dotted_name(name)
                .unwrap_or_else(|e| panic!("well_known constant {name:?} fails parse: {e}"));
        }
    }

    #[test]
    fn name_returns_expected_string_per_variant() {
        let cases: [(StateObservation, &str); 9] = [
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
                StateObservation::LndChannel(LndChannelState {
                    remote_pubkey: String::new(),
                    capacity_sat: 0,
                    local_balance_sat: 0,
                    remote_balance_sat: 0,
                    active: true,
                    private: false,
                    initiator: false,
                    csv_delay: 0,
                    commit_fee_sat: 0,
                    lifetime_seconds: 0,
                    last_update_height: None,
                    short_channel_id: None,
                    peer_online: None,
                }),
                well_known::LND_CHANNEL_DETAIL,
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
            assert_eq!(variant.name().as_str(), expected);
        }
    }
}
