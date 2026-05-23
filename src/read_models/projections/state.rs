//! Latest-state projection.
//!
//! Stores the latest [`StateObservation`] per `(subject, state name)`
//! and answers `StateReadModel`-shaped queries.

use std::collections::HashMap;

use crate::{
    observations::{Observation, ObservationPayload, StateName, StateObservation},
    read_models::{Projected, Projection, ProjectionError},
    shared::types::EntityRef,
};

#[derive(Debug, Default)]
pub struct StateProjection {
    by_key: HashMap<(EntityRef, StateName), Projected<StateObservation>>,
}

impl StateProjection {
    pub fn new() -> Self {
        Self::default()
    }

    /// Latest state of the given name for the given subject, or `None`
    /// if no observation has landed yet.
    pub fn get_latest(
        &self,
        subject: &EntityRef,
        name: &StateName,
    ) -> Option<Projected<StateObservation>> {
        self.by_key.get(&(subject.clone(), name.clone())).cloned()
    }

    /// All known states for a subject, one entry per state name.
    /// Iteration order is unspecified.
    pub fn for_subject(&self, subject: &EntityRef) -> Vec<Projected<StateObservation>> {
        self.by_key
            .iter()
            .filter(|((s, _), _)| s == subject)
            .map(|((_, _), v)| v.clone())
            .collect()
    }
}

impl Projection for StateProjection {
    fn apply(&mut self, obs: &Observation) -> Result<(), ProjectionError> {
        let state = match &obs.payload {
            ObservationPayload::State(s) => s,
            _ => return Ok(()),
        };

        let key = (obs.subject.clone(), state.name());
        if let Some(existing) = self.by_key.get(&key) {
            if existing.observed_at >= obs.observed_at {
                return Ok(());
            }
        }
        self.by_key.insert(
            key,
            Projected {
                value: state.clone(),
                observation_id: obs.id.clone(),
                observed_at: obs.observed_at,
            },
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::{CollectorRef, IntegrationKind};
    use crate::observations::{
        Attributes, BitcoinBlockchainState, BitcoinMempoolState, BitcoinNetworkState,
        BitcoinPeerSummaryState, HostState, LndChannelSummaryState, LndNodeState, LndWalletState,
        ObservationContext, ObservationOrigin, ObservationSource,
    };
    use crate::shared::types::{BitcoinNodeId, CollectorId, HostId, LndNodeId, SidecarId};
    use chrono::{Duration as ChronoDuration, TimeZone, Utc};
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn at(secs: i64) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 21, 12, 0, 0).unwrap() + ChronoDuration::seconds(secs)
    }

    fn ctx(subject: EntityRef, observed_at: chrono::DateTime<Utc>) -> ObservationContext {
        ObservationContext {
            source: ObservationSource {
                sidecar_id: SidecarId(Uuid::now_v7()),
                collector: CollectorRef {
                    id: CollectorId("test".into()),
                    integration: IntegrationKind::BitcoinCoreRpc {
                        interval: ChronoDuration::seconds(10),
                    },
                    instance_label: "test".into(),
                },
            },
            subject,
            observed_at,
            origin: ObservationOrigin::Collected,
        }
    }

    fn state_obs(
        subject: EntityRef,
        state: StateObservation,
        observed_at: chrono::DateTime<Utc>,
    ) -> Observation {
        Observation::state(
            ctx(subject, observed_at),
            state,
            Attributes(BTreeMap::new()),
        )
    }

    fn btc(s: &str) -> EntityRef {
        EntityRef::BitcoinNode(BitcoinNodeId(s.into()))
    }

    fn blockchain(blocks: u64) -> StateObservation {
        StateObservation::BitcoinBlockchain(BitcoinBlockchainState {
            chain: "main".into(),
            blocks,
            headers: blocks,
            best_block_hash: None,
            verification_progress: 1.0,
            initial_block_download: false,
            pruned: false,
            size_on_disk_bytes: 0,
        })
    }

    #[test]
    fn default_is_empty() {
        let p = StateProjection::default();
        let name =
            StateName::from_well_known(crate::observations::state::well_known::BITCOIN_BLOCKCHAIN);
        assert!(p.get_latest(&btc("alice"), &name).is_none());
    }

    #[test]
    fn apply_is_idempotent_for_identical_observations() {
        let mut p = StateProjection::new();
        let obs = state_obs(btc("alice"), blockchain(100), at(0));
        p.apply(&obs).unwrap();
        p.apply(&obs).unwrap();

        let latest = p
            .get_latest(&btc("alice"), &obs.payload_state_name())
            .unwrap();
        assert_eq!(latest.observation_id, obs.id);
        assert_eq!(latest.observed_at, obs.observed_at);
    }

    #[test]
    fn newer_observation_overwrites_older() {
        let mut p = StateProjection::new();
        let a = state_obs(btc("alice"), blockchain(100), at(0));
        let b = state_obs(btc("alice"), blockchain(200), at(60));
        p.apply(&a).unwrap();
        p.apply(&b).unwrap();

        let latest = p
            .get_latest(&btc("alice"), &a.payload_state_name())
            .unwrap();
        assert_eq!(latest.observation_id, b.id);
        match latest.value {
            StateObservation::BitcoinBlockchain(s) => assert_eq!(s.blocks, 200),
            _ => panic!(),
        }
    }

    #[test]
    fn older_observation_does_not_overwrite_newer() {
        let mut p = StateProjection::new();
        let newer = state_obs(btc("alice"), blockchain(200), at(60));
        let older = state_obs(btc("alice"), blockchain(100), at(0));
        p.apply(&newer).unwrap();
        p.apply(&older).unwrap();

        let latest = p
            .get_latest(&btc("alice"), &newer.payload_state_name())
            .unwrap();
        assert_eq!(latest.observation_id, newer.id);
        match latest.value {
            StateObservation::BitcoinBlockchain(s) => assert_eq!(s.blocks, 200),
            _ => panic!(),
        }
    }

    #[test]
    fn for_subject_returns_all_state_names_for_that_subject() {
        let mut p = StateProjection::new();
        p.apply(&state_obs(btc("alice"), blockchain(1), at(0)))
            .unwrap();
        p.apply(&state_obs(
            btc("alice"),
            StateObservation::BitcoinMempool(BitcoinMempoolState {
                loaded: true,
                tx_count: 1,
                bytes: 1,
                usage_bytes: 1,
                max_mempool_bytes: 1,
            }),
            at(1),
        ))
        .unwrap();
        p.apply(&state_obs(btc("bob"), blockchain(1), at(0)))
            .unwrap();

        let alice = p.for_subject(&btc("alice"));
        assert_eq!(alice.len(), 2);

        let bob = p.for_subject(&btc("bob"));
        assert_eq!(bob.len(), 1);
    }

    #[test]
    fn non_state_payload_is_ignored() {
        let mut p = StateProjection::new();
        let obs = Observation::metric(
            ctx(btc("alice"), at(0)),
            crate::observations::MetricName::parse("test.dummy").expect("valid"),
            crate::observations::MetricKind::Gauge,
            crate::observations::MetricValue::Numeric(crate::observations::NumericValue::U64(1)),
            crate::observations::Unit::Count,
            Attributes(BTreeMap::new()),
        );
        p.apply(&obs).unwrap();
        assert_eq!(p.by_key.len(), 0);
    }

    #[test]
    fn each_state_variant_roundtrips_through_apply_and_get_latest() {
        let mut p = StateProjection::new();
        let host = EntityRef::Host(HostId("h".into()));
        let lnd = EntityRef::LndNode(LndNodeId("ln".into()));

        let cases: Vec<(EntityRef, StateObservation)> = vec![
            (btc("alice"), blockchain(1)),
            (
                btc("alice"),
                StateObservation::BitcoinMempool(BitcoinMempoolState {
                    loaded: true,
                    tx_count: 0,
                    bytes: 0,
                    usage_bytes: 0,
                    max_mempool_bytes: 0,
                }),
            ),
            (
                btc("alice"),
                StateObservation::BitcoinNetwork(BitcoinNetworkState {
                    version: 0,
                    subversion: String::new(),
                    protocol_version: 0,
                    connections: 0,
                    connections_in: None,
                    connections_out: None,
                    network_active: None,
                }),
            ),
            (
                btc("alice"),
                StateObservation::BitcoinPeerSummary(BitcoinPeerSummaryState {
                    peer_count: 0,
                    inbound_count: None,
                    outbound_count: None,
                    block_relay_only_count: None,
                }),
            ),
            (
                lnd.clone(),
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
            ),
            (
                lnd.clone(),
                StateObservation::LndWallet(LndWalletState {
                    total_balance_sat: 0,
                    confirmed_balance_sat: 0,
                    unconfirmed_balance_sat: 0,
                }),
            ),
            (
                lnd.clone(),
                StateObservation::LndChannelSummary(LndChannelSummaryState {
                    active_channels: 0,
                    inactive_channels: 0,
                    pending_channels: 0,
                    total_capacity_sat: None,
                    local_balance_sat: 0,
                    remote_balance_sat: 0,
                    unsettled_balance_sat: None,
                }),
            ),
            (
                host.clone(),
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
            ),
        ];

        for (i, (subject, state)) in cases.iter().enumerate() {
            let obs = state_obs(subject.clone(), state.clone(), at(i as i64));
            p.apply(&obs).unwrap();

            let name = state.name();
            let latest = p.get_latest(subject, &name).expect("variant present");
            assert_eq!(latest.value.name(), name);
        }
    }

    /// Convenience to fetch the state name a State observation carries.
    /// Test-only — the projection itself derives the name in `apply`.
    trait PayloadStateName {
        fn payload_state_name(&self) -> StateName;
    }
    impl PayloadStateName for Observation {
        fn payload_state_name(&self) -> StateName {
            match &self.payload {
                ObservationPayload::State(s) => s.name(),
                _ => panic!("not a state observation"),
            }
        }
    }
}
