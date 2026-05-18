use chrono::{DateTime, Utc};

use crate::{
    incidents::{IncidentFingerprint, IncidentKind},
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
    /// Which incident kind this signal contributes to (ADR-L1 §§1–2).
    /// Validated by the engine against `KindRegistry` on receipt.
    pub kind: IncidentKind,
    /// Optional per-instance sub-key for kinds that need it
    /// (e.g. `payment_hash` for `lnd.htlc_stuck`, `mount_path` for
    /// `host.disk_exhaustion`). `None` for kinds whose `EntityRef` subject
    /// already gives full granularity.
    pub dimension: Option<String>,
    pub severity: SignalSeverity,
    pub status: SignalStatus,
    pub confidence: Confidence,
    pub evidence: Vec<EvidenceRef>,
}

/// Derive the structured fingerprint a draft would land on if accepted by
/// the engine. Per ADR-L1, the engine computes this on receipt; this helper
/// gives rules and tests a way to compute the same value off the engine's
/// hot path.
pub fn compute_fingerprint(draft: &IncidentSignalDraft) -> IncidentFingerprint {
    IncidentFingerprint {
        subject: draft.subject.clone(),
        kind: draft.kind.clone(),
        dimension: draft.dimension.clone(),
    }
}
