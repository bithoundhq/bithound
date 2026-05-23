//! Top-level envelope over per-context domain events (ADR-D4).
//!
//! V0 does not dispatch on an event bus — these enums are type-level
//! documentation for what crosses context boundaries, useful for
//! tracing today and cloud sync later. Each context owns its own
//! events module under `crate::<context>::events`; `DomainEvent`
//! sums them.

use serde::{Deserialize, Serialize};

use crate::diagnostics::events::DiagnosticEvent;
use crate::incidents::events::IncidentEvent;
use crate::notifications::events::NotificationEvent;
use crate::observations::events::ObservationEvent;
use crate::read_models::events::ReadModelEvent;

/// Sum over every per-context [`*Event`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomainEvent {
    Observation(ObservationEvent),
    ReadModel(ReadModelEvent),
    Diagnostic(DiagnosticEvent),
    Incident(IncidentEvent),
    Notification(NotificationEvent),
}

impl From<ObservationEvent> for DomainEvent {
    fn from(e: ObservationEvent) -> Self {
        DomainEvent::Observation(e)
    }
}

impl From<ReadModelEvent> for DomainEvent {
    fn from(e: ReadModelEvent) -> Self {
        DomainEvent::ReadModel(e)
    }
}

impl From<DiagnosticEvent> for DomainEvent {
    fn from(e: DiagnosticEvent) -> Self {
        DomainEvent::Diagnostic(e)
    }
}

impl From<IncidentEvent> for DomainEvent {
    fn from(e: IncidentEvent) -> Self {
        DomainEvent::Incident(e)
    }
}

impl From<NotificationEvent> for DomainEvent {
    fn from(e: NotificationEvent) -> Self {
        DomainEvent::Notification(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_each_variant_lifts_into_envelope() {
        let inc = IncidentEvent::DraftBelowConfidenceFloor {
            kind: crate::incidents::IncidentKind::parse("bitcoin.tip_lag").expect("valid"),
            confidence: crate::observations::Confidence::Low,
            floor: crate::observations::Confidence::Medium,
        };
        let env: DomainEvent = inc.into();
        assert!(matches!(env, DomainEvent::Incident(_)));

        let diag = DiagnosticEvent::RuleFailed {
            rule_id: "test".into(),
            error: "boom".into(),
        };
        let env: DomainEvent = diag.into();
        assert!(matches!(env, DomainEvent::Diagnostic(_)));
    }

    #[test]
    fn domain_event_round_trips_via_serde() {
        let env: DomainEvent = DiagnosticEvent::RuleFailed {
            rule_id: "rule-1".into(),
            error: "evaluate returned Err".into(),
        }
        .into();
        let json = serde_json::to_string(&env).expect("serialize");
        let back: DomainEvent = serde_json::from_str(&json).expect("deserialize");
        match back {
            DomainEvent::Diagnostic(DiagnosticEvent::RuleFailed { rule_id, error }) => {
                assert_eq!(rule_id, "rule-1");
                assert!(error.contains("evaluate"));
            }
            _ => panic!("unexpected variant after round-trip"),
        }
    }
}
