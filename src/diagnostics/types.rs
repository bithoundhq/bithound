use chrono::{DateTime, Utc};

use crate::{
    observations::{Confidence, SignalName, SignalSeverity, SignalStatus},
    read_models::{
        CapabilityReadModel, HealthReadModel, HeartbeatReadModel, MetricReadModel, StateReadModel,
    },
    shared::types::{EntityRef, EvidenceRef},
};

#[derive(Debug)]
pub struct DiagnosticContext<'a> {
    pub now: DateTime<Utc>,
    pub subject: &'a EntityRef,

    pub state: &'a dyn StateReadModel,
    pub metrics: &'a dyn MetricReadModel,
    pub health: &'a dyn HealthReadModel,
    pub capabilities: &'a dyn CapabilityReadModel,
    pub heartbeats: &'a dyn HeartbeatReadModel,
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
