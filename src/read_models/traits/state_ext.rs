//! Typed-helper extension trait for [`super::StateReadModel`].
//!
//! Per ADR-R1 §R1.3, this trait is auto-implemented for any
//! `StateReadModel`. It provides one helper per [`StateObservation`]
//! variant so diagnostic rules can avoid pattern-matching boilerplate
//! when they want a specific state shape.
//!
//! Adding a new state variant should add a matching helper here. The
//! parity test in `src/observations/types/state.rs` (BTH-6) catches
//! variant/constant drift; helper coverage here is a code-review
//! discipline, not type-enforced.

use crate::{
    observations::{
        self, BitcoinBlockchainState, BitcoinMempoolState, BitcoinNetworkState,
        BitcoinPeerSummaryState, HostState, LndChannelSummaryState, LndNodeState, LndWalletState,
        StateName, StateObservation,
    },
    read_models::Projected,
    shared::types::{BitcoinNodeId, EntityRef, HostId, LndNodeId},
};

use super::state::StateReadModel;

/// Per-variant typed accessors over a [`StateReadModel`].
///
/// Auto-implemented for every type that implements `StateReadModel`;
/// callers do not need to implement this trait themselves.
pub trait StateReadModelExt: StateReadModel {
    fn bitcoin_blockchain(
        &self,
        node: &BitcoinNodeId,
    ) -> Option<Projected<BitcoinBlockchainState>> {
        unwrap_state(self.latest_state(
            &EntityRef::BitcoinNode(node.clone()),
            &StateName(observations::state::well_known::BITCOIN_BLOCKCHAIN.to_string()),
        ))
    }

    fn bitcoin_mempool(&self, node: &BitcoinNodeId) -> Option<Projected<BitcoinMempoolState>> {
        unwrap_state(self.latest_state(
            &EntityRef::BitcoinNode(node.clone()),
            &StateName(observations::state::well_known::BITCOIN_MEMPOOL.to_string()),
        ))
    }

    fn bitcoin_network(&self, node: &BitcoinNodeId) -> Option<Projected<BitcoinNetworkState>> {
        unwrap_state(self.latest_state(
            &EntityRef::BitcoinNode(node.clone()),
            &StateName(observations::state::well_known::BITCOIN_NETWORK.to_string()),
        ))
    }

    fn bitcoin_peer_summary(
        &self,
        node: &BitcoinNodeId,
    ) -> Option<Projected<BitcoinPeerSummaryState>> {
        unwrap_state(self.latest_state(
            &EntityRef::BitcoinNode(node.clone()),
            &StateName(observations::state::well_known::BITCOIN_PEER_SUMMARY.to_string()),
        ))
    }

    fn lnd_node(&self, node: &LndNodeId) -> Option<Projected<LndNodeState>> {
        unwrap_state(self.latest_state(
            &EntityRef::LndNode(node.clone()),
            &StateName(observations::state::well_known::LND_NODE.to_string()),
        ))
    }

    fn lnd_wallet(&self, node: &LndNodeId) -> Option<Projected<LndWalletState>> {
        unwrap_state(self.latest_state(
            &EntityRef::LndNode(node.clone()),
            &StateName(observations::state::well_known::LND_WALLET.to_string()),
        ))
    }

    fn lnd_channel_summary(
        &self,
        node: &LndNodeId,
    ) -> Option<Projected<LndChannelSummaryState>> {
        unwrap_state(self.latest_state(
            &EntityRef::LndNode(node.clone()),
            &StateName(observations::state::well_known::LND_CHANNEL_SUMMARY.to_string()),
        ))
    }

    fn host_system(&self, host: &HostId) -> Option<Projected<HostState>> {
        unwrap_state(self.latest_state(
            &EntityRef::Host(host.clone()),
            &StateName(observations::state::well_known::HOST_SYSTEM.to_string()),
        ))
    }
}

impl<T: StateReadModel + ?Sized> StateReadModelExt for T {}

/// Trait used by the per-variant helpers to extract a typed payload from
/// a generic `Projected<StateObservation>`. Sealed via the private fn.
trait UnwrapStateVariant: Sized {
    fn from_state(state: StateObservation) -> Option<Self>;
}

impl UnwrapStateVariant for BitcoinBlockchainState {
    fn from_state(state: StateObservation) -> Option<Self> {
        match state {
            StateObservation::BitcoinBlockchain(s) => Some(s),
            _ => None,
        }
    }
}
impl UnwrapStateVariant for BitcoinMempoolState {
    fn from_state(state: StateObservation) -> Option<Self> {
        match state {
            StateObservation::BitcoinMempool(s) => Some(s),
            _ => None,
        }
    }
}
impl UnwrapStateVariant for BitcoinNetworkState {
    fn from_state(state: StateObservation) -> Option<Self> {
        match state {
            StateObservation::BitcoinNetwork(s) => Some(s),
            _ => None,
        }
    }
}
impl UnwrapStateVariant for BitcoinPeerSummaryState {
    fn from_state(state: StateObservation) -> Option<Self> {
        match state {
            StateObservation::BitcoinPeerSummary(s) => Some(s),
            _ => None,
        }
    }
}
impl UnwrapStateVariant for LndNodeState {
    fn from_state(state: StateObservation) -> Option<Self> {
        match state {
            StateObservation::LndNode(s) => Some(s),
            _ => None,
        }
    }
}
impl UnwrapStateVariant for LndWalletState {
    fn from_state(state: StateObservation) -> Option<Self> {
        match state {
            StateObservation::LndWallet(s) => Some(s),
            _ => None,
        }
    }
}
impl UnwrapStateVariant for LndChannelSummaryState {
    fn from_state(state: StateObservation) -> Option<Self> {
        match state {
            StateObservation::LndChannelSummary(s) => Some(s),
            _ => None,
        }
    }
}
impl UnwrapStateVariant for HostState {
    fn from_state(state: StateObservation) -> Option<Self> {
        match state {
            StateObservation::Host(s) => Some(s),
            _ => None,
        }
    }
}

fn unwrap_state<T: UnwrapStateVariant>(
    projected: Option<Projected<StateObservation>>,
) -> Option<Projected<T>> {
    let projected = projected?;
    let value = T::from_state(projected.value)?;
    Some(Projected {
        value,
        observation_id: projected.observation_id,
        observed_at: projected.observed_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::types::ObservationId;
    use chrono::{TimeZone, Utc};

    /// Minimal `StateReadModel` impl backed by a single hard-coded
    /// `(subject, name) → StateObservation` mapping.
    #[derive(Debug)]
    struct FakeStore {
        subject: EntityRef,
        name: StateName,
        value: StateObservation,
        observation_id: ObservationId,
        observed_at: chrono::DateTime<Utc>,
    }

    impl StateReadModel for FakeStore {
        fn latest_state(
            &self,
            subject: &EntityRef,
            name: &StateName,
        ) -> Option<Projected<StateObservation>> {
            if subject == &self.subject && name == &self.name {
                Some(Projected {
                    value: self.value.clone(),
                    observation_id: self.observation_id.clone(),
                    observed_at: self.observed_at,
                })
            } else {
                None
            }
        }

        fn states_for(&self, subject: &EntityRef) -> Vec<Projected<StateObservation>> {
            if subject == &self.subject {
                vec![Projected {
                    value: self.value.clone(),
                    observation_id: self.observation_id.clone(),
                    observed_at: self.observed_at,
                }]
            } else {
                vec![]
            }
        }
    }

    fn fixture_observed_at() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 18, 12, 0, 0).unwrap()
    }

    fn fixture_obs_id() -> ObservationId {
        ObservationId::new()
    }

    #[test]
    fn ext_helper_matches_latest_state_for_bitcoin_blockchain() {
        let node = BitcoinNodeId("alice".into());
        let inner = BitcoinBlockchainState {
            chain: "main".into(),
            blocks: 100,
            headers: 100,
            best_block_hash: None,
            verification_progress: 1.0,
            initial_block_download: false,
            pruned: false,
            size_on_disk_bytes: 0,
        };
        let obs_id = fixture_obs_id();
        let observed_at = fixture_observed_at();
        let store = FakeStore {
            subject: EntityRef::BitcoinNode(node.clone()),
            name: StateName(
                observations::state::well_known::BITCOIN_BLOCKCHAIN.to_string(),
            ),
            value: StateObservation::BitcoinBlockchain(inner.clone()),
            observation_id: obs_id.clone(),
            observed_at,
        };

        // Generic path.
        let via_latest = store
            .latest_state(
                &EntityRef::BitcoinNode(node.clone()),
                &StateName(observations::state::well_known::BITCOIN_BLOCKCHAIN.to_string()),
            )
            .expect("present");
        match via_latest.value {
            StateObservation::BitcoinBlockchain(ref s) => assert_eq!(s.blocks, 100),
            _ => panic!("wrong variant"),
        }

        // Typed-helper path.
        let via_helper = store.bitcoin_blockchain(&node).expect("present");
        assert_eq!(via_helper.value.blocks, 100);
        assert_eq!(via_helper.observation_id, obs_id);
        assert_eq!(via_helper.observed_at, observed_at);
    }

    #[test]
    fn ext_helper_returns_none_when_subject_does_not_match() {
        let node = BitcoinNodeId("alice".into());
        let other = BitcoinNodeId("bob".into());
        let store = FakeStore {
            subject: EntityRef::BitcoinNode(node.clone()),
            name: StateName(
                observations::state::well_known::BITCOIN_BLOCKCHAIN.to_string(),
            ),
            value: StateObservation::BitcoinBlockchain(BitcoinBlockchainState {
                chain: "main".into(),
                blocks: 1,
                headers: 1,
                best_block_hash: None,
                verification_progress: 1.0,
                initial_block_download: false,
                pruned: false,
                size_on_disk_bytes: 0,
            }),
            observation_id: fixture_obs_id(),
            observed_at: fixture_observed_at(),
        };
        assert!(store.bitcoin_blockchain(&other).is_none());
    }

    #[test]
    fn ext_helper_returns_none_when_variant_mismatches_name() {
        // The fake store says: at (alice, "bitcoin.blockchain") we have a
        // BitcoinMempool value (a contrived mismatch). The ext helper
        // unwraps based on the variant; with a BitcoinMempool stored at
        // the BitcoinBlockchain key, `bitcoin_blockchain()` returns None.
        let node = BitcoinNodeId("alice".into());
        let store = FakeStore {
            subject: EntityRef::BitcoinNode(node.clone()),
            name: StateName(
                observations::state::well_known::BITCOIN_BLOCKCHAIN.to_string(),
            ),
            value: StateObservation::BitcoinMempool(BitcoinMempoolState {
                loaded: true,
                tx_count: 0,
                bytes: 0,
                usage_bytes: 0,
                max_mempool_bytes: 0,
            }),
            observation_id: fixture_obs_id(),
            observed_at: fixture_observed_at(),
        };
        assert!(store.bitcoin_blockchain(&node).is_none());
    }

    #[test]
    fn ext_helpers_cover_all_state_variants() {
        // Smoke test: every helper returns `None` against an empty store
        // without panicking — i.e. each helper compiles, links to the
        // right StateName, and routes through unwrap_state correctly.
        struct EmptyStore;
        impl std::fmt::Debug for EmptyStore {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("EmptyStore")
            }
        }
        impl StateReadModel for EmptyStore {
            fn latest_state(
                &self,
                _subject: &EntityRef,
                _name: &StateName,
            ) -> Option<Projected<StateObservation>> {
                None
            }
            fn states_for(&self, _subject: &EntityRef) -> Vec<Projected<StateObservation>> {
                vec![]
            }
        }
        let store = EmptyStore;
        let btc = BitcoinNodeId("alice".into());
        let lnd = LndNodeId("lnd-alice".into());
        let host = HostId("host-alice".into());

        assert!(store.bitcoin_blockchain(&btc).is_none());
        assert!(store.bitcoin_mempool(&btc).is_none());
        assert!(store.bitcoin_network(&btc).is_none());
        assert!(store.bitcoin_peer_summary(&btc).is_none());
        assert!(store.lnd_node(&lnd).is_none());
        assert!(store.lnd_wallet(&lnd).is_none());
        assert!(store.lnd_channel_summary(&lnd).is_none());
        assert!(store.host_system(&host).is_none());
    }
}
