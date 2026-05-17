use serde::{Deserialize, Serialize};

use chrono::{DateTime, Utc};

use crate::shared::types::{EntityRef, EvidenceRef, IncidentId, ObservationId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Incident {
    pub id: IncidentId,
    pub kind: IncidentKind,
    pub subject: EntityRef,

    pub severity: IncidentSeverity,
    pub status: IncidentStatus,

    pub opened_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,

    pub signal_observation_ids: Vec<ObservationId>,
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
    /// Reserved for V0.2 — operator-acknowledged-known. Not set by the V0/V0.1 engine.
    /// V0.1 suppression is notifier-side via `SuppressionRule`; see ADR-L5.
    Suppressed,
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
