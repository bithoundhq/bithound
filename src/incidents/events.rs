use serde::Serialize;

use crate::{
    incidents::{Incident, IncidentKind, IncidentLifecycleEvent},
    observations::{Confidence, Observation},
};

/// Events emitted by [`IncidentEngine::handle`] in response to a single
/// [`IncidentCommand`]. Ordering within a single handle call is
/// `SignalRecorded` → `IncidentTouched` → `Lifecycle`; terminal-or-no-op
/// outcomes (e.g. `DraftBelowConfidenceFloor`) may appear anywhere.
///
/// `Serialize` but not `Deserialize`: events are produced by the engine
/// and consumed downstream; replay from storage must come back through
/// the command surface so it re-validates against the current registry.
#[derive(Debug, Clone, Serialize)]
pub enum IncidentEvent {
    SignalRecorded(Observation),
    IncidentTouched(Incident),
    Lifecycle(IncidentLifecycleEvent),
    DraftBelowConfidenceFloor {
        kind: IncidentKind,
        confidence: Confidence,
        floor: Confidence,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::incidents::IncidentKind;
    use crate::observations::Confidence;

    #[test]
    fn incident_event_serializes_to_json() {
        let ev = IncidentEvent::DraftBelowConfidenceFloor {
            kind: IncidentKind("bitcoin.tip_lag".into()),
            confidence: Confidence::Low,
            floor: Confidence::Medium,
        };
        let json = serde_json::to_string(&ev).expect("serialize");
        assert!(json.contains("DraftBelowConfidenceFloor"));
        assert!(json.contains("bitcoin.tip_lag"));
    }
}
