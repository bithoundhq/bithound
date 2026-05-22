//! Shared in-memory test fixtures for the bitcoin rule tests.
//!
//! `FakeReadModels` implements every read-model trait the
//! `DiagnosticContext` needs; rule tests populate only the projections
//! they care about (health, state) and leave the rest as `None`.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::incidents::IncidentKind;
use crate::observations::{
    CapabilityName, CapabilityObservation, HealthCheckObservation, HealthStatus, HealthTargetId,
    HeartbeatObservation, IncidentSignalObservation, MetricName, MetricObservation, SignalName,
    StateName, StateObservation,
};
use crate::read_models::{
    CapabilityReadModel, HealthReadModel, HeartbeatReadModel, IncidentSignalReadModel,
    MetricReadModel, Projected, StateReadModel,
};
use crate::shared::types::{EntityRef, ObservationId};

#[derive(Debug, Default)]
pub struct FakeReadModels {
    pub health: HashMap<(EntityRef, HealthTargetId), Projected<HealthCheckObservation>>,
    pub state: HashMap<(EntityRef, StateName), Projected<StateObservation>>,
}

impl FakeReadModels {
    pub fn set_health(
        &mut self,
        subject: &EntityRef,
        target: &str,
        status: HealthStatus,
        observed_at: DateTime<Utc>,
    ) {
        let target_id = HealthTargetId(target.into());
        self.health.insert(
            (subject.clone(), target_id.clone()),
            Projected {
                value: HealthCheckObservation {
                    target: target_id,
                    status,
                    latency_ms: None,
                    message: None,
                    error: None,
                },
                observation_id: ObservationId(Uuid::now_v7()),
                observed_at,
            },
        );
    }

    pub fn set_state(
        &mut self,
        subject: &EntityRef,
        state: StateObservation,
        observed_at: DateTime<Utc>,
    ) {
        let name = state.name();
        self.state.insert(
            (subject.clone(), name),
            Projected {
                value: state,
                observation_id: ObservationId(Uuid::now_v7()),
                observed_at,
            },
        );
    }
}

impl StateReadModel for FakeReadModels {
    fn latest_state(
        &self,
        subject: &EntityRef,
        name: &StateName,
    ) -> Option<Projected<StateObservation>> {
        self.state.get(&(subject.clone(), name.clone())).cloned()
    }

    fn states_for(&self, subject: &EntityRef) -> Vec<Projected<StateObservation>> {
        self.state
            .iter()
            .filter(|((s, _), _)| s == subject)
            .map(|((_, _), v)| v.clone())
            .collect()
    }
}

impl HealthReadModel for FakeReadModels {
    fn current_health(
        &self,
        subject: &EntityRef,
        target: &HealthTargetId,
    ) -> Option<Projected<HealthCheckObservation>> {
        self.health.get(&(subject.clone(), target.clone())).cloned()
    }
}

impl MetricReadModel for FakeReadModels {
    fn latest_metric(
        &self,
        _subject: &EntityRef,
        _name: &MetricName,
    ) -> Option<Projected<MetricObservation>> {
        None
    }
    fn metric_samples_since(
        &self,
        _subject: &EntityRef,
        _name: &MetricName,
        _since: DateTime<Utc>,
    ) -> Vec<Projected<MetricObservation>> {
        vec![]
    }
    fn unchanged_for(
        &self,
        _subject: &EntityRef,
        _name: &MetricName,
    ) -> Option<Vec<Projected<MetricObservation>>> {
        None
    }
}

impl CapabilityReadModel for FakeReadModels {
    fn current_capability(
        &self,
        _subject: &EntityRef,
        _capability: &CapabilityName,
    ) -> Option<Projected<CapabilityObservation>> {
        None
    }
    fn capabilities_for(&self, _subject: &EntityRef) -> Vec<Projected<CapabilityObservation>> {
        vec![]
    }
}

impl HeartbeatReadModel for FakeReadModels {
    fn latest_heartbeat(&self) -> Option<Projected<HeartbeatObservation>> {
        None
    }
    fn heartbeats_since(&self, _since: DateTime<Utc>) -> Vec<Projected<HeartbeatObservation>> {
        vec![]
    }
}

impl IncidentSignalReadModel for FakeReadModels {
    fn current_signal(
        &self,
        _subject: &EntityRef,
        _signal: &SignalName,
    ) -> Option<Projected<IncidentSignalObservation>> {
        None
    }
    fn active_signals_for(
        &self,
        _subject: &EntityRef,
    ) -> Vec<Projected<IncidentSignalObservation>> {
        vec![]
    }
    fn active_signals_for_incident_kind(
        &self,
        _subject: &EntityRef,
        _incident_kind: &IncidentKind,
    ) -> Vec<Projected<IncidentSignalObservation>> {
        vec![]
    }
}
