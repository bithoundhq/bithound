//! Latest capability projection.
//!
//! Stores the most recent [`CapabilityObservation`] per
//! `(subject, capability)`. Latest-write-wins by `observed_at`.

use std::collections::HashMap;

use crate::{
    observations::{CapabilityName, CapabilityObservation, Observation, ObservationPayload},
    read_models::{Projected, Projection, ProjectionError},
    shared::types::EntityRef,
};

#[derive(Debug, Default)]
pub struct CapabilityProjection {
    by_key: HashMap<(EntityRef, CapabilityName), Projected<CapabilityObservation>>,
}

impl CapabilityProjection {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current_capability(
        &self,
        subject: &EntityRef,
        capability: &CapabilityName,
    ) -> Option<Projected<CapabilityObservation>> {
        self.by_key
            .get(&(subject.clone(), capability.clone()))
            .cloned()
    }

    pub fn for_subject(&self, subject: &EntityRef) -> Vec<Projected<CapabilityObservation>> {
        self.by_key
            .iter()
            .filter(|((s, _), _)| s == subject)
            .map(|((_, _), v)| v.clone())
            .collect()
    }
}

impl Projection for CapabilityProjection {
    fn apply(&mut self, obs: &Observation) -> Result<(), ProjectionError> {
        let cap = match &obs.payload {
            ObservationPayload::Capability(c) => c,
            _ => return Ok(()),
        };
        let key = (obs.subject.clone(), cap.capability.clone());
        if let Some(existing) = self.by_key.get(&key) {
            if existing.observed_at >= obs.observed_at {
                return Ok(());
            }
        }
        self.by_key.insert(
            key,
            Projected {
                value: cap.clone(),
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
        Attributes, CapabilityStatus, ObservationContext, ObservationOrigin, ObservationSource,
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
                    id: CollectorId("test".into()),
                    integration: IntegrationKind::BitcoinCoreRpc {
                        interval: ChronoDuration::seconds(10),
                    },
                    instance_label: "test".into(),
                },
            },
            subject,
            observed_at,
            origin: ObservationOrigin::Collected,
        }
    }

    fn cap_obs(
        subject: EntityRef,
        name: &str,
        status: CapabilityStatus,
        observed_at: chrono::DateTime<Utc>,
    ) -> Observation {
        Observation::capability(
            ctx(subject, observed_at),
            CapabilityName::parse(name).expect("valid test capability name"),
            status,
            None,
            Attributes(BTreeMap::new()),
        )
    }

    fn btc(s: &str) -> EntityRef {
        EntityRef::BitcoinNode(BitcoinNodeId(s.into()))
    }

    #[test]
    fn default_is_empty() {
        let p = CapabilityProjection::default();
        let c = CapabilityName::parse("bitcoin.zmq.rawtx").expect("valid");
        assert!(p.current_capability(&btc("alice"), &c).is_none());
    }

    #[test]
    fn latest_write_wins() {
        let mut p = CapabilityProjection::new();
        let a = cap_obs(
            btc("alice"),
            "bitcoin.zmq.rawtx",
            CapabilityStatus::Available,
            t0(),
        );
        let b = cap_obs(
            btc("alice"),
            "bitcoin.zmq.rawtx",
            CapabilityStatus::Unavailable,
            t0() + ChronoDuration::seconds(60),
        );
        p.apply(&a).unwrap();
        p.apply(&b).unwrap();
        let cur = p
            .current_capability(
                &btc("alice"),
                &CapabilityName::parse("bitcoin.zmq.rawtx").expect("valid"),
            )
            .unwrap();
        assert_eq!(cur.value.status, CapabilityStatus::Unavailable);
        assert_eq!(cur.observation_id, b.id);
    }

    #[test]
    fn older_does_not_overwrite_newer() {
        let mut p = CapabilityProjection::new();
        let newer = cap_obs(
            btc("alice"),
            "bitcoin.zmq.rawtx",
            CapabilityStatus::Unavailable,
            t0() + ChronoDuration::seconds(60),
        );
        let older = cap_obs(
            btc("alice"),
            "bitcoin.zmq.rawtx",
            CapabilityStatus::Available,
            t0(),
        );
        p.apply(&newer).unwrap();
        p.apply(&older).unwrap();
        let cur = p
            .current_capability(
                &btc("alice"),
                &CapabilityName::parse("bitcoin.zmq.rawtx").expect("valid"),
            )
            .unwrap();
        assert_eq!(cur.value.status, CapabilityStatus::Unavailable);
    }

    #[test]
    fn for_subject_scans_only_that_subject() {
        let mut p = CapabilityProjection::new();
        p.apply(&cap_obs(
            btc("alice"),
            "bitcoin.zmq.rawtx",
            CapabilityStatus::Available,
            t0(),
        ))
        .unwrap();
        p.apply(&cap_obs(
            btc("alice"),
            "bitcoin.zmq.rawblock",
            CapabilityStatus::Available,
            t0(),
        ))
        .unwrap();
        p.apply(&cap_obs(
            btc("bob"),
            "bitcoin.zmq.rawtx",
            CapabilityStatus::Unavailable,
            t0(),
        ))
        .unwrap();

        assert_eq!(p.for_subject(&btc("alice")).len(), 2);
        assert_eq!(p.for_subject(&btc("bob")).len(), 1);
    }

    #[test]
    fn non_capability_payload_is_ignored() {
        let mut p = CapabilityProjection::new();
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
