use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::types::{EntityRef, EvidenceRef, ObservationId, Timestamp};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Incident {
    pub id: IncidentId,
    pub kind: IncidentKind,
    pub subject: EntityRef,

    pub severity: IncidentSeverity,
    pub status: IncidentStatus,

    pub opened_at: Timestamp,
    pub updated_at: Timestamp,
    pub resolved_at: Option<Timestamp>,

    pub signal_observation_ids: Option<ObservationId>,
    pub evidence: Vec<EvidenceRef>,

    pub summary: String,

    /// Optional durable display copy for retention purposes.
    pub evidence_summary: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IncidentId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub struct IncidentKind(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IncidentSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IncidentStatus {
    Open,
    Acknowledged,
    Resolved,
    Supressed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IncidentLifecycleEvent {
    Opened(Incident),
    Updated(Incident),
    Resolved(Incident),
}
