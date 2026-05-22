//! Types for incident signal observations.
//! Take note that these are not incidents per se.
use serde::{Deserialize, Serialize};

use crate::incidents::IncidentKind;
use crate::shared::types::*;

/// Incident signals are derived detection primitives.
/// They are not to be confused with full incidents themselves.
///
/// `incident_kind` records which incident kind this signal contributes
/// to, so the read-model layer can answer `active_signals_for_incident_kind`
/// without re-deriving the mapping. The engine populates it from the
/// originating `IncidentSignalDraft::kind`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentSignalObservation {
    pub signal: SignalName,
    pub incident_kind: IncidentKind,
    pub severity: SignalSeverity,
    pub status: SignalStatus,
    pub confidence: Confidence,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SignalName(pub String);

impl SignalName {
    /// Canonical signal name derived from an [`IncidentKind`]: the kind's
    /// dotted name with a `.signal` suffix.
    ///
    /// Centralizing the format here keeps the rule-to-signal-name mapping
    /// from drifting across the codebase. Tests and rules construct
    /// signal names via this helper rather than reformatting the suffix
    /// by hand.
    pub fn for_incident_kind(kind: &crate::incidents::IncidentKind) -> Self {
        SignalName(format!("{}.signal", kind.0))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalStatus {
    Active,
    Cleared,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Confidence {
    Low,
    Medium,
    High,
}
