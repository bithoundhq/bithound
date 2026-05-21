//! `ReadModelStore` — the assembler that holds every projection and
//! presents the six read-model trait surfaces by delegating to the
//! matching field.

use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::{
    incidents::IncidentKind,
    observations::{
        CapabilityName, CapabilityObservation, HealthCheckObservation, HealthTargetId,
        HeartbeatObservation, IncidentSignalObservation, MetricName, MetricObservation,
        Observation, ObservationPayload, SignalName, StateName, StateObservation,
    },
    read_models::{
        CapabilityProjection, CapabilityReadModel, HealthProjection, HealthReadModel,
        HeartbeatProjection, HeartbeatReadModel, IncidentSignalProjection,
        IncidentSignalReadModel, MetricProjection, MetricReadModel, Projected, Projection,
        ProjectionError, StateProjection, StateReadModel, DEFAULT_HEARTBEAT_CAPACITY,
        DEFAULT_METRIC_SERIES_CAPACITY,
    },
    shared::types::EntityRef,
};

#[derive(Debug, Clone, Copy)]
pub struct ReadModelStoreConfig {
    pub metric_series_capacity: usize,
    pub heartbeat_capacity: usize,
}

impl Default for ReadModelStoreConfig {
    fn default() -> Self {
        Self {
            metric_series_capacity: DEFAULT_METRIC_SERIES_CAPACITY,
            heartbeat_capacity: DEFAULT_HEARTBEAT_CAPACITY,
        }
    }
}

#[derive(Debug, Error)]
pub enum ApplyError {
    #[error("projection: {0}")]
    Projection(#[from] ProjectionError),
}

#[derive(Debug)]
pub struct ReadModelStore {
    pub state: StateProjection,
    pub metric: MetricProjection,
    pub health: HealthProjection,
    pub capability: CapabilityProjection,
    pub heartbeat: HeartbeatProjection,
    pub incident_signal: IncidentSignalProjection,
}

impl ReadModelStore {
    pub fn new(config: ReadModelStoreConfig) -> Self {
        Self {
            state: StateProjection::default(),
            metric: MetricProjection::with_capacity(config.metric_series_capacity),
            health: HealthProjection::default(),
            capability: CapabilityProjection::default(),
            heartbeat: HeartbeatProjection::with_capacity(config.heartbeat_capacity),
            incident_signal: IncidentSignalProjection::default(),
        }
    }

    /// Apply an observation to the matching projection.
    /// `Event`, `Inventory`, `Transition`, `Diagnosis` are no-ops in V0.
    pub fn apply(&mut self, obs: &Observation) -> Result<(), ApplyError> {
        match &obs.payload {
            ObservationPayload::State(_) => self.state.apply(obs)?,
            ObservationPayload::Metric(_) => self.metric.apply(obs)?,
            ObservationPayload::Health(_) => self.health.apply(obs)?,
            ObservationPayload::Capability(_) => self.capability.apply(obs)?,
            ObservationPayload::Heartbeat(_) => self.heartbeat.apply(obs)?,
            ObservationPayload::IncidentSignal(_) => self.incident_signal.apply(obs)?,
            ObservationPayload::Event(_)
            | ObservationPayload::Inventory(_)
            | ObservationPayload::Transition(_)
            | ObservationPayload::Diagnosis(_) => {}
        }
        Ok(())
    }
}

impl Default for ReadModelStore {
    fn default() -> Self {
        Self::new(ReadModelStoreConfig::default())
    }
}

impl StateReadModel for ReadModelStore {
    fn latest_state(
        &self,
        subject: &EntityRef,
        name: &StateName,
    ) -> Option<Projected<StateObservation>> {
        self.state.get_latest(subject, name)
    }

    fn states_for(&self, subject: &EntityRef) -> Vec<Projected<StateObservation>> {
        self.state.for_subject(subject)
    }
}

impl MetricReadModel for ReadModelStore {
    fn latest_metric(
        &self,
        subject: &EntityRef,
        name: &MetricName,
    ) -> Option<Projected<MetricObservation>> {
        self.metric.latest_metric(subject, name)
    }

    fn metric_samples_since(
        &self,
        subject: &EntityRef,
        name: &MetricName,
        since: DateTime<Utc>,
    ) -> Vec<Projected<MetricObservation>> {
        self.metric.metric_samples_since(subject, name, since)
    }

    fn unchanged_for(
        &self,
        subject: &EntityRef,
        name: &MetricName,
    ) -> Option<Vec<Projected<MetricObservation>>> {
        self.metric.unchanged_for(subject, name)
    }
}

impl HealthReadModel for ReadModelStore {
    fn current_health(
        &self,
        subject: &EntityRef,
        target: &HealthTargetId,
    ) -> Option<Projected<HealthCheckObservation>> {
        self.health.current_health(subject, target)
    }
}

impl CapabilityReadModel for ReadModelStore {
    fn current_capability(
        &self,
        subject: &EntityRef,
        capability: &CapabilityName,
    ) -> Option<Projected<CapabilityObservation>> {
        self.capability.current_capability(subject, capability)
    }

    fn capabilities_for(&self, subject: &EntityRef) -> Vec<Projected<CapabilityObservation>> {
        self.capability.for_subject(subject)
    }
}

impl HeartbeatReadModel for ReadModelStore {
    fn latest_heartbeat(&self) -> Option<Projected<HeartbeatObservation>> {
        self.heartbeat.latest_heartbeat()
    }

    fn heartbeats_since(&self, since: DateTime<Utc>) -> Vec<Projected<HeartbeatObservation>> {
        self.heartbeat.heartbeats_since(since)
    }
}

impl IncidentSignalReadModel for ReadModelStore {
    fn current_signal(
        &self,
        subject: &EntityRef,
        signal: &SignalName,
    ) -> Option<Projected<IncidentSignalObservation>> {
        self.incident_signal.current_signal(subject, signal)
    }

    fn active_signals_for(&self, subject: &EntityRef) -> Vec<Projected<IncidentSignalObservation>> {
        self.incident_signal.active_signals_for(subject)
    }

    fn active_signals_for_incident_kind(
        &self,
        subject: &EntityRef,
        incident_kind: &IncidentKind,
    ) -> Vec<Projected<IncidentSignalObservation>> {
        self.incident_signal
            .active_signals_for_incident_kind(subject, incident_kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::{CollectorRef, IntegrationKind};
    use crate::observations::{
        Attributes, BitcoinBlockchainState, CapabilityStatus, Confidence, HealthStatus,
        HeartbeatStatus, MetricKind, MetricValue, NumericValue, ObservationContext,
        ObservationOrigin, ObservationSource, SignalSeverity, SignalStatus, Unit,
    };
    use crate::shared::types::{BitcoinNodeId, CollectorId, SidecarId};
    use chrono::{Duration as ChronoDuration, TimeZone};
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn t0() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 21, 12, 0, 0).unwrap()
    }

    fn ctx(subject: EntityRef, observed_at: DateTime<Utc>) -> ObservationContext {
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

    fn btc(s: &str) -> EntityRef {
        EntityRef::BitcoinNode(BitcoinNodeId(s.into()))
    }

    #[test]
    fn apply_dispatches_each_payload_to_the_matching_projection() {
        let mut store = ReadModelStore::default();
        let alice = btc("alice");

        // State
        let state_obs = Observation::state(
            ctx(alice.clone(), t0()),
            StateObservation::BitcoinBlockchain(BitcoinBlockchainState {
                chain: "main".into(),
                blocks: 500,
                headers: 500,
                best_block_hash: None,
                verification_progress: 1.0,
                initial_block_download: false,
                pruned: false,
                size_on_disk_bytes: 0,
            }),
            Attributes(BTreeMap::new()),
        );
        store.apply(&state_obs).unwrap();

        // Metric
        let metric_obs = Observation::metric(
            ctx(alice.clone(), t0()),
            "peer_count",
            MetricKind::Gauge,
            MetricValue::Numeric(NumericValue::U64(8)),
            Unit::Count,
            Attributes(BTreeMap::new()),
        );
        store.apply(&metric_obs).unwrap();

        // Health
        let health_obs = Observation::health(
            ctx(alice.clone(), t0()),
            "bitcoin.rpc",
            HealthStatus::Ok,
            Some(12),
            None,
            None,
            Attributes(BTreeMap::new()),
        );
        store.apply(&health_obs).unwrap();

        // Capability
        let cap_obs = Observation::capability(
            ctx(alice.clone(), t0()),
            "bitcoin.zmq.rawtx",
            CapabilityStatus::Available,
            None,
            Attributes(BTreeMap::new()),
        );
        store.apply(&cap_obs).unwrap();

        // Heartbeat
        let hb_obs = Observation::heartbeat(
            ctx(alice.clone(), t0()),
            1,
            t0(),
            None,
            "0.0.1",
            HeartbeatStatus::Alive,
            vec![],
            Attributes(BTreeMap::new()),
        );
        store.apply(&hb_obs).unwrap();

        // Incident signal
        let signal_obs = Observation::incident_signal(
            ctx(alice.clone(), t0()),
            IncidentSignalObservation {
                signal: SignalName("bitcoin.tip_lag.signal".into()),
                incident_kind: IncidentKind("bitcoin.tip_lag".into()),
                severity: SignalSeverity::Warning,
                status: SignalStatus::Active,
                confidence: Confidence::High,
                evidence: vec![],
            },
            Attributes(BTreeMap::new()),
        );
        store.apply(&signal_obs).unwrap();

        // Now query via each trait surface.
        let state = StateReadModel::latest_state(
            &store,
            &alice,
            &StateName(crate::observations::state::well_known::BITCOIN_BLOCKCHAIN.to_string()),
        )
        .expect("state");
        match state.value {
            StateObservation::BitcoinBlockchain(s) => assert_eq!(s.blocks, 500),
            _ => panic!(),
        }

        let metric = MetricReadModel::latest_metric(
            &store,
            &alice,
            &MetricName("peer_count".into()),
        )
        .expect("metric");
        match metric.value.value {
            MetricValue::Numeric(NumericValue::U64(v)) => assert_eq!(v, 8),
            _ => panic!(),
        }

        let health = HealthReadModel::current_health(
            &store,
            &alice,
            &HealthTargetId("bitcoin.rpc".into()),
        )
        .expect("health");
        assert_eq!(health.value.status, HealthStatus::Ok);

        let cap = CapabilityReadModel::current_capability(
            &store,
            &alice,
            &CapabilityName("bitcoin.zmq.rawtx".into()),
        )
        .expect("capability");
        assert_eq!(cap.value.status, CapabilityStatus::Available);

        let hb = HeartbeatReadModel::latest_heartbeat(&store).expect("heartbeat");
        assert_eq!(hb.value.sequence, 1);

        let sig = IncidentSignalReadModel::current_signal(
            &store,
            &alice,
            &SignalName("bitcoin.tip_lag.signal".into()),
        )
        .expect("signal");
        assert_eq!(sig.value.status, SignalStatus::Active);

        let active_for_kind = IncidentSignalReadModel::active_signals_for_incident_kind(
            &store,
            &alice,
            &IncidentKind("bitcoin.tip_lag".into()),
        );
        assert_eq!(active_for_kind.len(), 1);

        // Delegation coverage for the remaining trait methods — each is a
        // 1-line proxy whose underlying projection method is exercised in
        // its own BTH-21..24 test module. These assertions cover the
        // delegation itself against populated state.
        let states = StateReadModel::states_for(&store, &alice);
        assert_eq!(states.len(), 1);

        let samples = MetricReadModel::metric_samples_since(
            &store,
            &alice,
            &MetricName("peer_count".into()),
            t0() - ChronoDuration::seconds(1),
        );
        assert_eq!(samples.len(), 1);

        let unchanged = MetricReadModel::unchanged_for(
            &store,
            &alice,
            &MetricName("peer_count".into()),
        )
        .expect("series populated");
        assert_eq!(unchanged.len(), 1);

        let caps = CapabilityReadModel::capabilities_for(&store, &alice);
        assert_eq!(caps.len(), 1);

        let hbs = HeartbeatReadModel::heartbeats_since(
            &store,
            t0() - ChronoDuration::seconds(1),
        );
        assert_eq!(hbs.len(), 1);

        let active = IncidentSignalReadModel::active_signals_for(&store, &alice);
        assert_eq!(active.len(), 1);
    }

    #[test]
    fn no_op_payloads_do_not_touch_any_projection() {
        let mut store = ReadModelStore::default();
        let alice = btc("alice");

        // Event payload — no projection cares.
        let event_obs = Observation::event(
            ctx(alice.clone(), t0()),
            "ibd_started",
            crate::observations::EventSeverity::Info,
            None,
            Attributes(BTreeMap::new()),
        );
        store.apply(&event_obs).unwrap();

        // Transition payload — no projection cares.
        let transition_obs = Observation::transition(
            ctx(alice.clone(), t0()),
            "ibd",
            crate::observations::StateAtom::String("active".into()),
            crate::observations::StateAtom::String("done".into()),
            None,
            Attributes(BTreeMap::new()),
        );
        store.apply(&transition_obs).unwrap();

        // Inventory payload — no projection cares.
        let inventory_obs = Observation::inventory(
            ctx(alice.clone(), t0()),
            "facts",
            BTreeMap::new(),
            Attributes(BTreeMap::new()),
        );
        store.apply(&inventory_obs).unwrap();

        // Diagnosis payload — no projection cares.
        let diag = crate::observations::DiagnosisObservation {
            diagnosis: crate::observations::DiagnosisName("x".into()),
            summary: "y".into(),
            confidence: Confidence::Medium,
            likely_causes: vec![],
            recommended_actions: vec![],
            evidence: vec![],
        };
        let diag_obs = Observation::diagnosis(ctx(alice, t0()), diag, Attributes(BTreeMap::new()));
        store.apply(&diag_obs).unwrap();

        // All projections empty.
        assert!(StateReadModel::states_for(&store, &btc("alice")).is_empty());
        assert!(MetricReadModel::latest_metric(
            &store,
            &btc("alice"),
            &MetricName("x".into())
        )
        .is_none());
        assert!(HealthReadModel::current_health(
            &store,
            &btc("alice"),
            &HealthTargetId("x".into())
        )
        .is_none());
        assert!(CapabilityReadModel::capabilities_for(&store, &btc("alice")).is_empty());
        assert!(HeartbeatReadModel::latest_heartbeat(&store).is_none());
        assert!(
            IncidentSignalReadModel::active_signals_for(&store, &btc("alice")).is_empty()
        );
    }

    #[test]
    fn config_threads_capacity_to_projections() {
        let store = ReadModelStore::new(ReadModelStoreConfig {
            metric_series_capacity: 7,
            heartbeat_capacity: 11,
        });
        assert_eq!(store.metric.capacity(), 7);
        assert_eq!(store.heartbeat.capacity(), 11);
    }
}
