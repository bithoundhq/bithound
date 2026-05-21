//! Incident engine — fingerprinting, lifecycle, command handling.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::{
    diagnostics::types::IncidentSignalDraft,
    incidents::{kinds::DraftError, kinds::KindRegistry, Incident, IncidentFingerprint},
    shared::types::{ActorId, IncidentId, SidecarId},
};

#[derive(Debug, Clone)]
pub enum IncidentCommand {
    RecordSignal(IncidentSignalDraft),
    Acknowledge {
        id: IncidentId,
        by: ActorId,
        at: DateTime<Utc>,
    },
    Resolve {
        id: IncidentId,
        by: ActorId,
        at: DateTime<Utc>,
        reason: String,
    },
}

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("draft validation: {0}")]
    Draft(#[from] DraftError),
    #[error("command not yet implemented: {0}")]
    NotYetImplemented(&'static str),
}

/// Single-writer incident state.
///
/// `open_incidents` is the authoritative in-memory map of currently-open
/// incidents keyed by [`IncidentFingerprint`]. It is rebuilt at startup
/// from the incident repository and then mutated only through
/// [`IncidentEngine::handle`].
pub struct IncidentEngine {
    kinds: KindRegistry,
    sidecar_id: SidecarId,
    open_incidents: HashMap<IncidentFingerprint, Incident>,
}

impl IncidentEngine {
    /// Build the engine from a kind registry, a sidecar identity, and the
    /// open incidents loaded from durable storage.
    ///
    /// Panics if `open_incidents` contains two incidents with the same
    /// fingerprint. The fingerprint is the engine's primary key for open
    /// incidents, so a duplicate indicates corruption in the persistence
    /// layer rather than a recoverable state.
    pub fn new(kinds: KindRegistry, sidecar_id: SidecarId, open_incidents: Vec<Incident>) -> Self {
        let mut map: HashMap<IncidentFingerprint, Incident> =
            HashMap::with_capacity(open_incidents.len());
        for incident in open_incidents {
            let fp = incident.fingerprint.clone();
            if map.insert(fp.clone(), incident).is_some() {
                panic!(
                    "IncidentEngine::new: duplicate fingerprint in open incidents: {}",
                    fp.as_key()
                );
            }
        }
        Self {
            kinds,
            sidecar_id,
            open_incidents: map,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::incidents::{IncidentFingerprint, IncidentKind, IncidentSeverity, IncidentStatus};
    use crate::shared::types::{BitcoinNodeId, EntityRef, IncidentId, ObservationId, SidecarId};
    use chrono::TimeZone;
    use uuid::Uuid;

    fn registry() -> KindRegistry {
        let toml = r#"
[[kinds]]
name = "bitcoin.tip_lag"
allowed_subjects = ["BitcoinNode"]
allows_dimension = false
"#;
        KindRegistry::load_from_toml_strs(toml, None).expect("load")
    }

    fn fp(kind: &str) -> IncidentFingerprint {
        IncidentFingerprint {
            subject: EntityRef::BitcoinNode(BitcoinNodeId("alice".into())),
            kind: IncidentKind(kind.into()),
            dimension: None,
        }
    }

    fn incident(kind: &str) -> Incident {
        let fingerprint = fp(kind);
        let now = chrono::Utc.with_ymd_and_hms(2026, 5, 21, 12, 0, 0).unwrap();
        Incident {
            id: IncidentId(Uuid::now_v7()),
            fingerprint: fingerprint.clone(),
            kind: fingerprint.kind.clone(),
            subject: fingerprint.subject.clone(),
            severity: IncidentSeverity::Warning,
            status: IncidentStatus::Open,
            opened_at: now,
            updated_at: now,
            resolved_at: None,
            signal_observation_ids: vec![ObservationId::new()],
            evidence: vec![],
            summary: "test incident".into(),
            evidence_summary: vec![],
        }
    }

    fn sidecar() -> SidecarId {
        SidecarId(Uuid::now_v7())
    }

    #[test]
    fn new_with_no_open_incidents_yields_empty_map() {
        let engine = IncidentEngine::new(registry(), sidecar(), vec![]);
        assert_eq!(engine.open_incidents.len(), 0);
    }

    #[test]
    fn new_indexes_open_incidents_by_fingerprint() {
        let a = incident("bitcoin.tip_lag");
        let mut b = incident("bitcoin.tip_lag");
        b.fingerprint = IncidentFingerprint {
            subject: EntityRef::BitcoinNode(BitcoinNodeId("bob".into())),
            kind: IncidentKind("bitcoin.tip_lag".into()),
            dimension: None,
        };
        b.subject = b.fingerprint.subject.clone();

        let engine = IncidentEngine::new(registry(), sidecar(), vec![a.clone(), b.clone()]);

        assert_eq!(engine.open_incidents.len(), 2);
        assert!(engine.open_incidents.contains_key(&a.fingerprint));
        assert!(engine.open_incidents.contains_key(&b.fingerprint));
    }

    #[test]
    #[should_panic(expected = "duplicate fingerprint")]
    fn new_panics_on_duplicate_fingerprints() {
        let a = incident("bitcoin.tip_lag");
        let b = incident("bitcoin.tip_lag");
        let _ = IncidentEngine::new(registry(), sidecar(), vec![a, b]);
    }
}
