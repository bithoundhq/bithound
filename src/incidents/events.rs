use serde::{Deserialize, Serialize};

use crate::{
    incidents::{Incident, IncidentKind, IncidentLifecycleEvent},
    observations::{Confidence, Observation},
};

/// Events emitted by [`IncidentEngine::handle`] in response to a single
/// [`IncidentCommand`]. Ordering within a single handle call is
/// `SignalRecorded` → `IncidentTouched` → `Lifecycle`; terminal-or-no-op
/// outcomes (e.g. `DraftBelowConfidenceFloor`) may appear anywhere.
///
/// Both `Serialize` and `Deserialize` per ADR-D4 § "cloud-sync-ready".
/// Re-deserializing an event does *not* replay it through the engine —
/// any replay must come back through the command surface so it
/// re-validates against the current registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
            kind: IncidentKind::parse("bitcoin.tip_lag").expect("valid test kind"),
            confidence: Confidence::Low,
            floor: Confidence::Medium,
        };
        let json = serde_json::to_string(&ev).expect("serialize");
        assert!(json.contains("DraftBelowConfidenceFloor"));
        assert!(json.contains("bitcoin.tip_lag"));
    }
}
