//! Latest health-check projection.
//!
//! Stores the most recent [`HealthCheckObservation`] per
//! `(subject, target)`. Latest-write-wins by `observed_at`.

use std::collections::HashMap;

use crate::{
    observations::{HealthCheckObservation, HealthTargetId, Observation, ObservationPayload},
    read_models::{Projected, Projection, ProjectionError},
    shared::types::EntityRef,
};

#[derive(Debug, Default)]
pub struct HealthProjection {
    by_key: HashMap<(EntityRef, HealthTargetId), Projected<HealthCheckObservation>>,
}

impl HealthProjection {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current_health(
        &self,
        subject: &EntityRef,
        target: &HealthTargetId,
    ) -> Option<Projected<HealthCheckObservation>> {
        self.by_key.get(&(subject.clone(), target.clone())).cloned()
    }

    pub fn for_subject(&self, subject: &EntityRef) -> Vec<Projected<HealthCheckObservation>> {
        self.by_key
            .iter()
            .filter_map(|((s, _), v)| (s == subject).then(|| v.clone()))
            .collect()
    }
}

impl Projection for HealthProjection {
    fn apply(&mut self, obs: &Observation) -> Result<(), ProjectionError> {
        let health = match &obs.payload {
            ObservationPayload::Health(h) => h,
            _ => return Ok(()),
        };
        let key = (obs.subject.clone(), health.target.clone());
        if let Some(existing) = self.by_key.get(&key) {
            if existing.observed_at >= obs.observed_at {
                return Ok(());
            }
        }
        self.by_key.insert(
            key,
            Projected {
                value: health.clone(),
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
        Attributes, HealthStatus, ObservationContext, ObservationOrigin, ObservationSource,
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

    fn health_obs(
        subject: EntityRef,
        target: &str,
        status: HealthStatus,
        observed_at: chrono::DateTime<Utc>,
    ) -> Observation {
        Observation::health(
            ctx(subject, observed_at),
            target,
            status,
            None,
            None,
            None,
            Attributes(BTreeMap::new()),
        )
    }

    fn btc(s: &str) -> EntityRef {
        EntityRef::BitcoinNode(BitcoinNodeId(s.into()))
    }

    #[test]
    fn default_is_empty() {
        let p = HealthProjection::default();
        let t = HealthTargetId("bitcoin.rpc".into());
        assert!(p.current_health(&btc("alice"), &t).is_none());
    }

    #[test]
    fn latest_write_wins() {
        let mut p = HealthProjection::new();
        let a = health_obs(btc("alice"), "bitcoin.rpc", HealthStatus::Ok, t0());
        let b = health_obs(
            btc("alice"),
            "bitcoin.rpc",
            HealthStatus::Critical,
            t0() + ChronoDuration::seconds(60),
        );
        p.apply(&a).unwrap();
        p.apply(&b).unwrap();
        let cur = p
            .current_health(&btc("alice"), &HealthTargetId("bitcoin.rpc".into()))
            .unwrap();
        assert_eq!(cur.value.status, HealthStatus::Critical);
        assert_eq!(cur.observation_id, b.id);
    }

    #[test]
    fn older_does_not_overwrite_newer() {
        let mut p = HealthProjection::new();
        let newer = health_obs(
            btc("alice"),
            "bitcoin.rpc",
            HealthStatus::Critical,
            t0() + ChronoDuration::seconds(60),
        );
        let older = health_obs(btc("alice"), "bitcoin.rpc", HealthStatus::Ok, t0());
        p.apply(&newer).unwrap();
        p.apply(&older).unwrap();
        let cur = p
            .current_health(&btc("alice"), &HealthTargetId("bitcoin.rpc".into()))
            .unwrap();
        assert_eq!(cur.value.status, HealthStatus::Critical);
    }

    #[test]
    fn for_subject_scans_only_that_subject() {
        let mut p = HealthProjection::new();
        p.apply(&health_obs(btc("alice"), "rpc", HealthStatus::Ok, t0())).unwrap();
        p.apply(&health_obs(btc("alice"), "zmq", HealthStatus::Ok, t0())).unwrap();
        p.apply(&health_obs(btc("bob"), "rpc", HealthStatus::Critical, t0()))
            .unwrap();

        let alice = p.for_subject(&btc("alice"));
        assert_eq!(alice.len(), 2);
        let bob = p.for_subject(&btc("bob"));
        assert_eq!(bob.len(), 1);
    }

    #[test]
    fn non_health_payload_is_ignored() {
        let mut p = HealthProjection::new();
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
