use serde::{Deserialize, Serialize};

use crate::shared::types::{EntityRef, EvidenceRef, IncidentId, ObservationId, Timestamp};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IncidentNotificationEventKind {
    Opened,
    Escalated,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IncidentLifecycleEvent {
    Opened(Incident),
    Escalated {
        incident: Incident,
        previous_severity: IncidentSeverity,
        new_severity: IncidentSeverity,
    },
    Resolved(Incident),
}

impl IncidentLifecycleEvent {
    pub fn notification_kind(&self) -> IncidentNotificationEventKind {
        match self {
            IncidentLifecycleEvent::Opened(_) => IncidentNotificationEventKind::Opened,
            IncidentLifecycleEvent::Escalated { .. } => IncidentNotificationEventKind::Escalated,
            IncidentLifecycleEvent::Resolved(_) => IncidentNotificationEventKind::Resolved,
        }
    }

    pub fn incident(&self) -> &Incident {
        match self {
            IncidentLifecycleEvent::Opened(incident)
            | IncidentLifecycleEvent::Escalated { incident, .. }
            | IncidentLifecycleEvent::Resolved(incident) => incident,
        }
    }
}
