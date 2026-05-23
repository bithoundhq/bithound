//! Incident engine — fingerprinting, lifecycle, command handling.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::{
    diagnostics::types::{compute_fingerprint, IncidentSignalDraft},
    incidents::{
        events::IncidentEvent,
        kinds::{DraftError, KindRegistry},
        Incident, IncidentFingerprint, IncidentLifecycleEvent, IncidentSeverity, IncidentStatus,
    },
    observations::{
        Attributes, IncidentSignalObservation, Observation, ObservationContext, ObservationOrigin,
        ObservationSource, SignalSeverity, SignalStatus,
    },
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
///
/// `signal_source` is the [`ObservationSource`] stamped on every
/// `IncidentSignal` observation the engine produces. The runtime
/// synthesizes a stable engine-local source at startup so signal
/// observations carry their producer identity.
pub struct IncidentEngine {
    kinds: KindRegistry,
    #[allow(dead_code)] // wired in by future commands; recorded for completeness.
    sidecar_id: SidecarId,
    signal_source: ObservationSource,
    open_incidents: HashMap<IncidentFingerprint, Incident>,
}

impl IncidentEngine {
    /// Build the engine from its dependencies and the open incidents
    /// loaded from durable storage.
    ///
    /// Panics if `open_incidents` contains two incidents with the same
    /// fingerprint. The fingerprint is the engine's primary key for open
    /// incidents, so a duplicate indicates corruption in the persistence
    /// layer rather than a recoverable state.
    pub fn new(
        kinds: KindRegistry,
        sidecar_id: SidecarId,
        signal_source: ObservationSource,
        open_incidents: Vec<Incident>,
    ) -> Self {
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
            signal_source,
            open_incidents: map,
        }
    }

    /// Apply a command and return the resulting event stream.
    ///
    /// Within a single call, events are emitted in side-effect order:
    /// `SignalRecorded` → `IncidentTouched` → `Lifecycle`. Terminal
    /// outcomes like `DraftBelowConfidenceFloor` follow the
    /// `SignalRecorded` they pair with.
    pub fn handle(
        &mut self,
        cmd: IncidentCommand,
        now: DateTime<Utc>,
    ) -> Result<Vec<IncidentEvent>, EngineError> {
        match cmd {
            IncidentCommand::RecordSignal(draft) => self.handle_record_signal(draft, now),
            IncidentCommand::Acknowledge { .. } => {
                Err(EngineError::NotYetImplemented("Acknowledge"))
            }
            IncidentCommand::Resolve { .. } => Err(EngineError::NotYetImplemented("Resolve")),
        }
    }

    fn handle_record_signal(
        &mut self,
        draft: IncidentSignalDraft,
        now: DateTime<Utc>,
    ) -> Result<Vec<IncidentEvent>, EngineError> {
        self.kinds.validate_draft(&draft)?;

        let spec = self
            .kinds
            .lookup(&draft.kind)
            .expect("validated draft has a registered kind");
        let floor = spec.min_open_confidence.clone();

        let fingerprint = compute_fingerprint(&draft);
        let observation = self.build_signal_observation(&draft, now);
        let observation_id = observation.id.clone();

        let mut events = Vec::with_capacity(3);
        events.push(IncidentEvent::SignalRecorded(observation));

        match draft.status {
            SignalStatus::Active => {
                if confidence_rank(&draft.confidence) < confidence_rank(&floor) {
                    events.push(IncidentEvent::DraftBelowConfidenceFloor {
                        kind: draft.kind.clone(),
                        confidence: draft.confidence.clone(),
                        floor,
                    });
                    return Ok(events);
                }

                let draft_severity = severity_from_signal(&draft.severity);

                if let Some(existing) = self.open_incidents.get_mut(&fingerprint) {
                    let prev_severity = existing.severity.clone();
                    let new_severity = max_severity(&prev_severity, &draft_severity);

                    existing.severity = new_severity.clone();
                    existing.updated_at = now;
                    existing.signal_observation_ids.push(observation_id);
                    existing.evidence.extend(draft.evidence.iter().cloned());

                    let touched = existing.clone();
                    events.push(IncidentEvent::IncidentTouched(touched.clone()));

                    if severity_rank(&new_severity) > severity_rank(&prev_severity) {
                        events.push(IncidentEvent::Lifecycle(
                            IncidentLifecycleEvent::Escalated {
                                incident: touched,
                                previous_severity: prev_severity,
                                new_severity,
                            },
                        ));
                    }
                } else {
                    let new_incident = Incident {
                        id: IncidentId::new(),
                        fingerprint: fingerprint.clone(),
                        kind: draft.kind.clone(),
                        subject: draft.subject.clone(),
                        severity: draft_severity,
                        status: IncidentStatus::Open,
                        opened_at: now,
                        updated_at: now,
                        resolved_at: None,
                        signal_observation_ids: vec![observation_id],
                        evidence: draft.evidence.clone(),
                        summary: draft.signal.as_str().to_string(),
                        evidence_summary: vec![],
                    };
                    self.open_incidents
                        .insert(fingerprint.clone(), new_incident.clone());
                    events.push(IncidentEvent::IncidentTouched(new_incident.clone()));
                    events.push(IncidentEvent::Lifecycle(IncidentLifecycleEvent::Opened(
                        new_incident,
                    )));
                }
            }
            SignalStatus::Cleared => {
                if let Some(mut existing) = self.open_incidents.remove(&fingerprint) {
                    existing.status = IncidentStatus::Resolved;
                    existing.updated_at = now;
                    existing.resolved_at = Some(now);
                    existing.signal_observation_ids.push(observation_id);

                    events.push(IncidentEvent::IncidentTouched(existing.clone()));
                    events.push(IncidentEvent::Lifecycle(IncidentLifecycleEvent::Resolved(
                        existing,
                    )));
                }
            }
        }

        Ok(events)
    }

    fn build_signal_observation(
        &self,
        draft: &IncidentSignalDraft,
        now: DateTime<Utc>,
    ) -> Observation {
        let ctx = ObservationContext {
            source: self.signal_source.clone(),
            subject: draft.subject.clone(),
            observed_at: now,
            origin: ObservationOrigin::Computed,
        };
        let signal = IncidentSignalObservation {
            signal: draft.signal.clone(),
            incident_kind: draft.kind.clone(),
            severity: draft.severity.clone(),
            status: draft.status.clone(),
            confidence: draft.confidence.clone(),
            evidence: draft.evidence.clone(),
        };
        Observation::incident_signal(ctx, signal, Attributes(std::collections::BTreeMap::new()))
    }
}

fn severity_from_signal(s: &SignalSeverity) -> IncidentSeverity {
    match s {
        SignalSeverity::Info => IncidentSeverity::Info,
        SignalSeverity::Warning => IncidentSeverity::Warning,
        SignalSeverity::Critical => IncidentSeverity::Critical,
    }
}

fn severity_rank(s: &IncidentSeverity) -> u8 {
    match s {
        IncidentSeverity::Info => 0,
        IncidentSeverity::Warning => 1,
        IncidentSeverity::Critical => 2,
    }
}

fn max_severity(a: &IncidentSeverity, b: &IncidentSeverity) -> IncidentSeverity {
    if severity_rank(a) >= severity_rank(b) {
        a.clone()
    } else {
        b.clone()
    }
}

fn confidence_rank(c: &crate::observations::Confidence) -> u8 {
    use crate::observations::Confidence;
    match c {
        Confidence::Low => 0,
        Confidence::Medium => 1,
        Confidence::High => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::{CollectorRef, IntegrationKind};
    use crate::incidents::{IncidentFingerprint, IncidentKind, IncidentSeverity, IncidentStatus};
    use crate::observations::{Confidence, ObservationPayload, ObservationSource, SignalName};
    use crate::shared::types::{
        BitcoinNodeId, CollectorId, EntityRef, EvidenceRef, IncidentId, LndNodeId, ObservationId,
        SidecarId,
    };
    use chrono::{Duration as ChronoDuration, TimeZone};
    use uuid::Uuid;

    const KINDS_TOML: &str = r#"
[[kinds]]
name = "bitcoin.tip_lag"
allowed_subjects = ["BitcoinNode"]
allows_dimension = false
min_open_confidence = "Medium"

[[kinds]]
name = "bitcoin.peer_starvation"
allowed_subjects = ["BitcoinNode"]
allows_dimension = false
min_open_confidence = "High"
"#;

    fn registry() -> KindRegistry {
        KindRegistry::load_from_toml_strs(KINDS_TOML, None).expect("load")
    }

    fn sidecar() -> SidecarId {
        SidecarId(Uuid::now_v7())
    }

    fn source(sidecar_id: &SidecarId) -> ObservationSource {
        ObservationSource {
            sidecar_id: sidecar_id.clone(),
            collector: CollectorRef {
                id: CollectorId("test-engine".into()),
                integration: IntegrationKind::BitcoinCoreRpc {
                    interval: ChronoDuration::seconds(10),
                },
                instance_label: "test".into(),
            },
        }
    }

    fn engine() -> IncidentEngine {
        let sid = sidecar();
        let src = source(&sid);
        IncidentEngine::new(registry(), sid, src, vec![])
    }

    fn engine_with(open: Vec<Incident>) -> IncidentEngine {
        let sid = sidecar();
        let src = source(&sid);
        IncidentEngine::new(registry(), sid, src, open)
    }

    fn now() -> DateTime<Utc> {
        chrono::Utc.with_ymd_and_hms(2026, 5, 21, 12, 0, 0).unwrap()
    }

    fn fp_tip_lag() -> IncidentFingerprint {
        IncidentFingerprint {
            subject: EntityRef::BitcoinNode(BitcoinNodeId("alice".into())),
            kind: IncidentKind::parse("bitcoin.tip_lag").expect("valid test kind"),
            dimension: None,
        }
    }

    fn draft(
        severity: SignalSeverity,
        status: SignalStatus,
        confidence: Confidence,
    ) -> IncidentSignalDraft {
        IncidentSignalDraft {
            subject: EntityRef::BitcoinNode(BitcoinNodeId("alice".into())),
            signal: SignalName::parse("bitcoin.tip_lag.signal").expect("valid"),
            kind: IncidentKind::parse("bitcoin.tip_lag").expect("valid test kind"),
            dimension: None,
            severity,
            status,
            confidence,
            evidence: vec![],
        }
    }

    fn open_incident(fingerprint: IncidentFingerprint, severity: IncidentSeverity) -> Incident {
        let t = now() - ChronoDuration::minutes(5);
        Incident {
            id: IncidentId(Uuid::now_v7()),
            fingerprint: fingerprint.clone(),
            kind: fingerprint.kind.clone(),
            subject: fingerprint.subject.clone(),
            severity,
            status: IncidentStatus::Open,
            opened_at: t,
            updated_at: t,
            resolved_at: None,
            signal_observation_ids: vec![ObservationId::new()],
            evidence: vec![],
            summary: "pre-existing".into(),
            evidence_summary: vec![],
        }
    }

    // ── BTH-18 carry-over (engine construction) ─────────────────────────

    #[test]
    fn new_with_no_open_incidents_yields_empty_map() {
        let engine = engine();
        assert_eq!(engine.open_incidents.len(), 0);
    }

    #[test]
    fn new_indexes_open_incidents_by_fingerprint() {
        let a = open_incident(fp_tip_lag(), IncidentSeverity::Warning);
        let other_fp = IncidentFingerprint {
            subject: EntityRef::BitcoinNode(BitcoinNodeId("bob".into())),
            kind: IncidentKind::parse("bitcoin.tip_lag").expect("valid test kind"),
            dimension: None,
        };
        let b = open_incident(other_fp.clone(), IncidentSeverity::Warning);

        let engine = engine_with(vec![a.clone(), b.clone()]);
        assert_eq!(engine.open_incidents.len(), 2);
        assert!(engine.open_incidents.contains_key(&a.fingerprint));
        assert!(engine.open_incidents.contains_key(&b.fingerprint));
    }

    #[test]
    #[should_panic(expected = "duplicate fingerprint")]
    fn new_panics_on_duplicate_fingerprints() {
        let a = open_incident(fp_tip_lag(), IncidentSeverity::Warning);
        let b = open_incident(fp_tip_lag(), IncidentSeverity::Critical);
        let _ = engine_with(vec![a, b]);
    }

    // ── handle: validation ──────────────────────────────────────────────

    #[test]
    fn rejects_unknown_kind_with_engine_error_and_no_events() {
        let mut e = engine();
        let mut d = draft(
            SignalSeverity::Warning,
            SignalStatus::Active,
            Confidence::High,
        );
        d.kind = IncidentKind::parse("bitcoin.nonexistent").expect("valid test kind");

        let err = e
            .handle(IncidentCommand::RecordSignal(d), now())
            .unwrap_err();
        assert!(matches!(
            err,
            EngineError::Draft(DraftError::UnknownKind(_))
        ));
        assert_eq!(e.open_incidents.len(), 0);
    }

    #[test]
    fn rejects_disallowed_subject_with_engine_error_and_no_events() {
        let mut e = engine();
        let mut d = draft(
            SignalSeverity::Warning,
            SignalStatus::Active,
            Confidence::High,
        );
        d.subject = EntityRef::LndNode(LndNodeId("ln".into()));

        let err = e
            .handle(IncidentCommand::RecordSignal(d), now())
            .unwrap_err();
        assert!(matches!(
            err,
            EngineError::Draft(DraftError::DisallowedSubject { .. })
        ));
    }

    // ── handle: Active draft, no open incident ──────────────────────────

    #[test]
    fn active_draft_opens_new_incident_when_above_floor() {
        let mut e = engine();
        let d = draft(
            SignalSeverity::Warning,
            SignalStatus::Active,
            Confidence::High,
        );
        let events = e
            .handle(IncidentCommand::RecordSignal(d), now())
            .expect("ok");

        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], IncidentEvent::SignalRecorded(_)));
        assert!(matches!(events[1], IncidentEvent::IncidentTouched(_)));
        assert!(matches!(
            events[2],
            IncidentEvent::Lifecycle(IncidentLifecycleEvent::Opened(_))
        ));

        let opened_incident = match &events[1] {
            IncidentEvent::IncidentTouched(i) => i,
            _ => unreachable!(),
        };
        assert_eq!(opened_incident.severity, IncidentSeverity::Warning);
        assert_eq!(opened_incident.status, IncidentStatus::Open);
        assert_eq!(opened_incident.opened_at, now());
        assert_eq!(opened_incident.updated_at, now());
        assert!(opened_incident.resolved_at.is_none());
        assert_eq!(e.open_incidents.len(), 1);
    }

    #[test]
    fn active_draft_below_floor_emits_no_lift_no_lifecycle() {
        let mut e = engine();
        let mut d = draft(
            SignalSeverity::Warning,
            SignalStatus::Active,
            Confidence::Low,
        );
        d.kind = IncidentKind::parse("bitcoin.peer_starvation").expect("valid test kind");
        d.signal = SignalName::parse("bitcoin.peer_starvation.signal").expect("valid");

        let events = e
            .handle(IncidentCommand::RecordSignal(d), now())
            .expect("ok");

        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], IncidentEvent::SignalRecorded(_)));
        match &events[1] {
            IncidentEvent::DraftBelowConfidenceFloor {
                kind,
                confidence,
                floor,
            } => {
                assert_eq!(
                    *kind,
                    IncidentKind::parse("bitcoin.peer_starvation").expect("valid")
                );
                assert_eq!(*confidence, Confidence::Low);
                assert_eq!(*floor, Confidence::High);
            }
            _ => panic!("expected DraftBelowConfidenceFloor"),
        }
        assert_eq!(e.open_incidents.len(), 0);
    }

    // ── handle: Active draft, existing open incident ────────────────────

    #[test]
    fn active_draft_on_open_incident_with_unchanged_severity_bumps_updated_at_only() {
        let pre = open_incident(fp_tip_lag(), IncidentSeverity::Warning);
        let pre_updated_at = pre.updated_at;
        let mut e = engine_with(vec![pre]);

        let d = draft(
            SignalSeverity::Warning,
            SignalStatus::Active,
            Confidence::High,
        );
        let events = e
            .handle(IncidentCommand::RecordSignal(d), now())
            .expect("ok");

        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], IncidentEvent::SignalRecorded(_)));
        let touched = match &events[1] {
            IncidentEvent::IncidentTouched(i) => i,
            _ => panic!("expected IncidentTouched"),
        };
        assert_eq!(touched.severity, IncidentSeverity::Warning);
        assert!(touched.updated_at > pre_updated_at);
        assert_eq!(touched.signal_observation_ids.len(), 2);
    }

    #[test]
    fn active_draft_escalates_severity_when_strictly_greater() {
        let pre = open_incident(fp_tip_lag(), IncidentSeverity::Warning);
        let mut e = engine_with(vec![pre]);

        let d = draft(
            SignalSeverity::Critical,
            SignalStatus::Active,
            Confidence::High,
        );
        let events = e
            .handle(IncidentCommand::RecordSignal(d), now())
            .expect("ok");

        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], IncidentEvent::SignalRecorded(_)));
        let touched = match &events[1] {
            IncidentEvent::IncidentTouched(i) => i,
            _ => panic!("expected IncidentTouched"),
        };
        assert_eq!(touched.severity, IncidentSeverity::Critical);

        match &events[2] {
            IncidentEvent::Lifecycle(IncidentLifecycleEvent::Escalated {
                previous_severity,
                new_severity,
                ..
            }) => {
                assert_eq!(*previous_severity, IncidentSeverity::Warning);
                assert_eq!(*new_severity, IncidentSeverity::Critical);
            }
            other => panic!("expected Lifecycle::Escalated, got {:?}", other),
        }
    }

    #[test]
    fn active_draft_does_not_de_escalate_severity() {
        let pre = open_incident(fp_tip_lag(), IncidentSeverity::Critical);
        let mut e = engine_with(vec![pre]);

        let d = draft(SignalSeverity::Info, SignalStatus::Active, Confidence::High);
        let events = e
            .handle(IncidentCommand::RecordSignal(d), now())
            .expect("ok");

        assert_eq!(events.len(), 2, "no Lifecycle event for de-escalation");
        let touched = match &events[1] {
            IncidentEvent::IncidentTouched(i) => i,
            _ => panic!("expected IncidentTouched"),
        };
        assert_eq!(touched.severity, IncidentSeverity::Critical);
    }

    #[test]
    fn active_draft_appends_evidence_to_existing_incident() {
        let pre = open_incident(fp_tip_lag(), IncidentSeverity::Warning);
        let mut e = engine_with(vec![pre]);

        let extra = EvidenceRef(ObservationId::new());
        let mut d = draft(
            SignalSeverity::Warning,
            SignalStatus::Active,
            Confidence::High,
        );
        d.evidence = vec![extra.clone()];

        let events = e
            .handle(IncidentCommand::RecordSignal(d), now())
            .expect("ok");

        let touched = match &events[1] {
            IncidentEvent::IncidentTouched(i) => i,
            _ => panic!(),
        };
        assert!(touched.evidence.iter().any(|e| e == &extra));
    }

    // ── handle: Cleared draft ───────────────────────────────────────────

    #[test]
    fn cleared_draft_on_open_incident_resolves_it() {
        let pre = open_incident(fp_tip_lag(), IncidentSeverity::Warning);
        let mut e = engine_with(vec![pre]);

        let d = draft(
            SignalSeverity::Info,
            SignalStatus::Cleared,
            Confidence::High,
        );
        let events = e
            .handle(IncidentCommand::RecordSignal(d), now())
            .expect("ok");

        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], IncidentEvent::SignalRecorded(_)));
        let touched = match &events[1] {
            IncidentEvent::IncidentTouched(i) => i,
            _ => panic!(),
        };
        assert_eq!(touched.status, IncidentStatus::Resolved);
        assert_eq!(touched.resolved_at, Some(now()));

        assert!(matches!(
            events[2],
            IncidentEvent::Lifecycle(IncidentLifecycleEvent::Resolved(_))
        ));
        assert_eq!(
            e.open_incidents.len(),
            0,
            "resolved incidents leave the open map"
        );
    }

    #[test]
    fn cleared_draft_with_no_open_incident_emits_only_signal_recorded() {
        let mut e = engine();
        let d = draft(
            SignalSeverity::Info,
            SignalStatus::Cleared,
            Confidence::High,
        );

        let events = e
            .handle(IncidentCommand::RecordSignal(d), now())
            .expect("ok");

        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], IncidentEvent::SignalRecorded(_)));
    }

    // ── handle: re-opening on a previously-resolved fingerprint ────────

    #[test]
    fn active_draft_after_resolution_opens_a_brand_new_incident() {
        let mut e = engine();
        let d_open = draft(
            SignalSeverity::Warning,
            SignalStatus::Active,
            Confidence::High,
        );
        e.handle(IncidentCommand::RecordSignal(d_open), now())
            .expect("open");

        let d_clear = draft(
            SignalSeverity::Info,
            SignalStatus::Cleared,
            Confidence::High,
        );
        e.handle(
            IncidentCommand::RecordSignal(d_clear),
            now() + ChronoDuration::seconds(60),
        )
        .expect("resolve");

        assert_eq!(e.open_incidents.len(), 0);

        let d_reopen = draft(
            SignalSeverity::Warning,
            SignalStatus::Active,
            Confidence::High,
        );
        let events = e
            .handle(
                IncidentCommand::RecordSignal(d_reopen),
                now() + ChronoDuration::seconds(120),
            )
            .expect("reopen");

        assert_eq!(events.len(), 3);
        assert!(matches!(
            events[2],
            IncidentEvent::Lifecycle(IncidentLifecycleEvent::Opened(_))
        ));
        assert_eq!(e.open_incidents.len(), 1);
    }

    // ── handle: Acknowledge / Resolve stubs ─────────────────────────────

    #[test]
    fn acknowledge_returns_not_yet_implemented() {
        let mut e = engine();
        let cmd = IncidentCommand::Acknowledge {
            id: IncidentId::new(),
            by: ActorId::operator("op"),
            at: now(),
        };
        match e.handle(cmd, now()).unwrap_err() {
            EngineError::NotYetImplemented(name) => assert_eq!(name, "Acknowledge"),
            other => panic!("expected NotYetImplemented, got {:?}", other),
        }
    }

    #[test]
    fn resolve_returns_not_yet_implemented() {
        let mut e = engine();
        let cmd = IncidentCommand::Resolve {
            id: IncidentId::new(),
            by: ActorId::operator("op"),
            at: now(),
            reason: "ack".into(),
        };
        match e.handle(cmd, now()).unwrap_err() {
            EngineError::NotYetImplemented(name) => assert_eq!(name, "Resolve"),
            other => panic!("expected NotYetImplemented, got {:?}", other),
        }
    }

    // ── handle: signal observation shape ────────────────────────────────

    #[test]
    fn signal_recorded_observation_has_computed_origin_and_engine_subject() {
        let mut e = engine();
        let d = draft(
            SignalSeverity::Warning,
            SignalStatus::Active,
            Confidence::High,
        );
        let events = e
            .handle(IncidentCommand::RecordSignal(d.clone()), now())
            .expect("ok");

        let obs = match &events[0] {
            IncidentEvent::SignalRecorded(o) => o,
            _ => panic!(),
        };
        assert_eq!(obs.origin, ObservationOrigin::Computed);
        assert_eq!(obs.subject, d.subject);
        assert_eq!(obs.observed_at, now());
        assert!(matches!(obs.payload, ObservationPayload::IncidentSignal(_)));
    }
}
