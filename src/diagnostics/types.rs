use chrono::{DateTime, Utc};

use crate::{
    observations::{Confidence, SignalName, SignalSeverity, SignalStatus},
    read_models::{
        CapabilityReadModel, HealthReadModel, HeartbeatReadModel, IncidentSignalReadModel,
        MetricReadModel, StateReadModel,
    },
    shared::types::{EntityRef, EvidenceRef},
};

/// Read-model trait references handed to a `DiagnosticRule::evaluate`
/// invocation. Per ADR-001 §4, this includes a `signals` reference so
/// rules emitting `SignalStatus::Cleared` can check the
/// `IncidentSignalReadModel` for the currently-active state — rules
/// must not clear signals they never raised.
#[derive(Debug)]
pub struct DiagnosticContext<'a> {
    pub now: DateTime<Utc>,
    pub subject: &'a EntityRef,

    pub state: &'a dyn StateReadModel,
    pub metrics: &'a dyn MetricReadModel,
    pub health: &'a dyn HealthReadModel,
    pub capabilities: &'a dyn CapabilityReadModel,
    pub heartbeats: &'a dyn HeartbeatReadModel,
    pub signals: &'a dyn IncidentSignalReadModel,
}

#[derive(Debug, Clone)]
pub struct IncidentSignalDraft {
    pub subject: EntityRef,
    pub signal: SignalName,
    pub severity: SignalSeverity,
    pub status: SignalStatus,
    pub confidence: Confidence,
    pub evidence: Vec<EvidenceRef>,
}
