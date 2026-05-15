//! Types for incident signal observations.
//! Take note that these are not incidents per se.
use serde::{Deserialize, Serialize};

use crate::shared::types::*;

/// Incident signals are derived detection primitives.
/// They are not to be confused with full incidents themselves.
/// # Example
/// ```
/// IncidentSignalObservation {
///     signal: SignalName("bitcoin.no_peers".into()),
///     severity: SignalSeverity::Critical,
///     status: SignalStatus::Active,
///     confidence: Confidence::High,
///     evidence: vec![
///         EvidenceRef(peer_count_metric_observation_id),
///         EvidenceRef(bitcoin_network_state_observation_id)
///     ]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentSignalObservation {
    pub signal: SignalName,
    pub severity: SignalSeverity,
    pub status: SignalStatus,
    pub confidence: Confidence,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SignalName(pub String);

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
