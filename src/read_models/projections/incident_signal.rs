//! Latest incident-signal projection.
//!
//! Stores the most recent [`IncidentSignalObservation`] per
//! `(subject, signal name)`. Supports the three queries on
//! `IncidentSignalReadModel`: current per `(subject, signal)`, all
//! active signals for a subject, and active signals filtered by
//! [`IncidentKind`].

use std::collections::HashMap;

use crate::{
    incidents::IncidentKind,
    observations::{
        IncidentSignalObservation, Observation, ObservationPayload, SignalName, SignalStatus,
    },
    read_models::{Projected, Projection, ProjectionError},
    shared::types::EntityRef,
};

#[derive(Debug, Default)]
pub struct IncidentSignalProjection {
    by_key: HashMap<(EntityRef, SignalName), Projected<IncidentSignalObservation>>,
}

impl IncidentSignalProjection {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current_signal(
        &self,
        subject: &EntityRef,
        signal: &SignalName,
    ) -> Option<Projected<IncidentSignalObservation>> {
        self.by_key.get(&(subject.clone(), signal.clone())).cloned()
    }

    pub fn active_signals_for(
        &self,
        subject: &EntityRef,
    ) -> Vec<Projected<IncidentSignalObservation>> {
        self.by_key
            .iter()
            .filter(|((s, _), v)| s == subject && v.value.status == SignalStatus::Active)
            .map(|(_, v)| v.clone())
            .collect()
    }

    pub fn active_signals_for_incident_kind(
        &self,
        subject: &EntityRef,
        incident_kind: &IncidentKind,
    ) -> Vec<Projected<IncidentSignalObservation>> {
        self.by_key
            .iter()
            .filter(|((s, _), v)| {
                s == subject
                    && v.value.status == SignalStatus::Active
                    && &v.value.incident_kind == incident_kind
            })
            .map(|(_, v)| v.clone())
            .collect()
    }
}

impl Projection for IncidentSignalProjection {
    fn apply(&mut self, obs: &Observation) -> Result<(), ProjectionError> {
        let signal = match &obs.payload {
            ObservationPayload::IncidentSignal(s) => s,
            _ => return Ok(()),
        };
        let key = (obs.subject.clone(), signal.signal.clone());
        if let Some(existing) = self.by_key.get(&key) {
            if existing.observed_at >= obs.observed_at {
                return Ok(());
            }
        }
        self.by_key.insert(
            key,
            Projected {
                value: signal.clone(),
                observation_id: obs.id.clone(),
                observed_at: obs.observed_at,
            },
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::{CollectorRef, IntegrationKind};
    use crate::observations::{
        Attributes, Confidence, ObservationContext, ObservationOrigin, ObservationSource,
        SignalSeverity,
    };
    use crate::shared::types::{BitcoinNodeId, CollectorId, SidecarId};
    use chrono::{Duration as ChronoDuration, TimeZone, Utc};
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn t0() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 21, 12, 0, 0).unwrap()
    }

    fn ctx(subject: EntityRef, observed_at: chrono::DateTime<Utc>) -> ObservationContext {
        ObservationContext {
            source: ObservationSource {
                sidecar_id: SidecarId(Uuid::now_v7()),
                collector: CollectorRef {
                    id: CollectorId("engine".into()),
                    integration: IntegrationKind::BitcoinCoreRpc {
                        interval: ChronoDuration::seconds(10),
                    },
                    instance_label: "engine".into(),
                },
            },
            subject,
            observed_at,
            origin: ObservationOrigin::Computed,
        }
    }

    fn signal_obs(
        subject: EntityRef,
        signal: &str,
        incident_kind: &str,
        status: SignalStatus,
        observed_at: chrono::DateTime<Utc>,
    ) -> Observation {
        let payload = IncidentSignalObservation {
            signal: SignalName::parse(signal).expect("valid test signal name"),
            incident_kind: IncidentKind::parse(incident_kind).expect("valid test kind"),
            severity: SignalSeverity::Warning,
            status,
            confidence: Confidence::High,
            evidence: vec![],
        };
        Observation::incident_signal(
            ctx(subject, observed_at),
            payload,
            Attributes(BTreeMap::new()),
        )
    }

    fn btc(s: &str) -> EntityRef {
        EntityRef::BitcoinNode(BitcoinNodeId(s.into()))
    }

    #[test]
    fn default_is_empty() {
        let p = IncidentSignalProjection::default();
        assert!(p
            .current_signal(
                &btc("alice"),
                &SignalName::parse("test.missing").expect("valid"),
            )
            .is_none());
    }

    #[test]
    fn latest_write_wins() {
        let mut p = IncidentSignalProjection::new();
        let a = signal_obs(
            btc("alice"),
            "bitcoin.tip_lag.signal",
            "bitcoin.tip_lag",
            SignalStatus::Active,
            t0(),
        );
        let b = signal_obs(
            btc("alice"),
            "bitcoin.tip_lag.signal",
            "bitcoin.tip_lag",
            SignalStatus::Cleared,
            t0() + ChronoDuration::seconds(60),
        );
        p.apply(&a).unwrap();
        p.apply(&b).unwrap();
        let cur = p
            .current_signal(
                &btc("alice"),
                &SignalName::parse("bitcoin.tip_lag.signal").expect("valid"),
            )
            .unwrap();
        assert_eq!(cur.value.status, SignalStatus::Cleared);
    }

    #[test]
    fn active_signals_for_excludes_cleared() {
        let mut p = IncidentSignalProjection::new();
        p.apply(&signal_obs(
            btc("alice"),
            "bitcoin.tip_lag.signal",
            "bitcoin.tip_lag",
            SignalStatus::Active,
            t0(),
        ))
        .unwrap();
        p.apply(&signal_obs(
            btc("alice"),
            "bitcoin.peer_starvation.signal",
            "bitcoin.peer_starvation",
            SignalStatus::Cleared,
            t0(),
        ))
        .unwrap();

        let active = p.active_signals_for(&btc("alice"));
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].value.signal.as_str(), "bitcoin.tip_lag.signal");
    }

    #[test]
    fn active_signals_for_filters_by_subject() {
        let mut p = IncidentSignalProjection::new();
        p.apply(&signal_obs(
            btc("alice"),
            "bitcoin.tip_lag.signal",
            "bitcoin.tip_lag",
            SignalStatus::Active,
            t0(),
        ))
        .unwrap();
        p.apply(&signal_obs(
            btc("bob"),
            "bitcoin.tip_lag.signal",
            "bitcoin.tip_lag",
            SignalStatus::Active,
            t0(),
        ))
        .unwrap();

        assert_eq!(p.active_signals_for(&btc("alice")).len(), 1);
        assert_eq!(p.active_signals_for(&btc("bob")).len(), 1);
    }

    #[test]
    fn active_signals_for_incident_kind_filters_by_kind() {
        let mut p = IncidentSignalProjection::new();
        p.apply(&signal_obs(
            btc("alice"),
            "bitcoin.tip_lag.signal",
            "bitcoin.tip_lag",
            SignalStatus::Active,
            t0(),
        ))
        .unwrap();
        p.apply(&signal_obs(
            btc("alice"),
            "bitcoin.peer_starvation.signal",
            "bitcoin.peer_starvation",
            SignalStatus::Active,
            t0(),
        ))
        .unwrap();

        let tip_lag = p.active_signals_for_incident_kind(
            &btc("alice"),
            &IncidentKind::parse("bitcoin.tip_lag").expect("valid test kind"),
        );
        assert_eq!(tip_lag.len(), 1);
        assert_eq!(tip_lag[0].value.incident_kind.as_str(), "bitcoin.tip_lag");
    }

    #[test]
    fn active_signals_for_incident_kind_excludes_cleared() {
        let mut p = IncidentSignalProjection::new();
        p.apply(&signal_obs(
            btc("alice"),
            "bitcoin.tip_lag.signal",
            "bitcoin.tip_lag",
            SignalStatus::Cleared,
            t0(),
        ))
        .unwrap();
        let tip_lag = p.active_signals_for_incident_kind(
            &btc("alice"),
            &IncidentKind::parse("bitcoin.tip_lag").expect("valid test kind"),
        );
        assert!(tip_lag.is_empty());
    }

    #[test]
    fn older_does_not_overwrite_newer() {
        let mut p = IncidentSignalProjection::new();
        let newer = signal_obs(
            btc("alice"),
            "bitcoin.tip_lag.signal",
            "bitcoin.tip_lag",
            SignalStatus::Cleared,
            t0() + ChronoDuration::seconds(60),
        );
        let older = signal_obs(
            btc("alice"),
            "bitcoin.tip_lag.signal",
            "bitcoin.tip_lag",
            SignalStatus::Active,
            t0(),
        );
        p.apply(&newer).unwrap();
        p.apply(&older).unwrap();
        let cur = p
            .current_signal(
                &btc("alice"),
                &SignalName::parse("bitcoin.tip_lag.signal").expect("valid"),
            )
            .unwrap();
        assert_eq!(cur.value.status, SignalStatus::Cleared);
    }

    #[test]
    fn non_signal_payload_is_ignored() {
        let mut p = IncidentSignalProjection::new();
        let obs = Observation::state(
            ctx(btc("alice"), t0()),
            crate::observations::StateObservation::BitcoinMempool(
                crate::observations::BitcoinMempoolState {
                    loaded: true,
                    tx_count: 0,
                    bytes: 0,
                    usage_bytes: 0,
                    max_mempool_bytes: 0,
                },
            ),
            Attributes(BTreeMap::new()),
        );
        p.apply(&obs).unwrap();
        assert!(p.by_key.is_empty());
    }
}
