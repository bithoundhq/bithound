//! Sidecar heartbeat projection.
//!
//! Sidecar-scoped (no per-subject key). Keeps the most recent
//! heartbeat plus a bounded history ring of recent heartbeats.

use std::collections::VecDeque;

use chrono::{DateTime, Utc};

use crate::{
    observations::{HeartbeatObservation, Observation, ObservationPayload},
    read_models::{Projected, Projection, ProjectionError},
};

pub const DEFAULT_HEARTBEAT_CAPACITY: usize = 256;

#[derive(Debug)]
pub struct HeartbeatProjection {
    capacity: usize,
    history: VecDeque<Projected<HeartbeatObservation>>,
}

impl Default for HeartbeatProjection {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_HEARTBEAT_CAPACITY)
    }
}

impl HeartbeatProjection {
    /// Panics if `capacity` is zero.
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(capacity > 0, "HeartbeatProjection capacity must be > 0");
        Self {
            capacity,
            history: VecDeque::new(),
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn latest_heartbeat(&self) -> Option<Projected<HeartbeatObservation>> {
        self.history.back().cloned()
    }

    pub fn heartbeats_since(&self, since: DateTime<Utc>) -> Vec<Projected<HeartbeatObservation>> {
        self.history
            .iter()
            .filter(|p| p.observed_at > since)
            .cloned()
            .collect()
    }
}

impl Projection for HeartbeatProjection {
    fn apply(&mut self, obs: &Observation) -> Result<(), ProjectionError> {
        let hb = match &obs.payload {
            ObservationPayload::Heartbeat(h) => h,
            _ => return Ok(()),
        };
        self.history.push_back(Projected {
            value: hb.clone(),
            observation_id: obs.id.clone(),
            observed_at: obs.observed_at,
        });
        while self.history.len() > self.capacity {
            self.history.pop_front();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::{CollectorRef, IntegrationKind};
    use crate::observations::{
        Attributes, HeartbeatStatus, ObservationContext, ObservationOrigin, ObservationSource,
    };
    use crate::shared::types::{BitcoinNodeId, CollectorId, EntityRef, SidecarId};
    use chrono::{Duration as ChronoDuration, TimeZone};
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn t0() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 21, 12, 0, 0).unwrap()
    }

    fn ctx(observed_at: DateTime<Utc>) -> ObservationContext {
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
            // Heartbeats nominally have a sidecar-shaped subject; we
            // reuse a BitcoinNode here because the projection doesn't
            // key by subject.
            subject: EntityRef::BitcoinNode(BitcoinNodeId("test".into())),
            observed_at,
            origin: ObservationOrigin::Computed,
        }
    }

    fn hb(seq: u64, observed_at: DateTime<Utc>) -> Observation {
        Observation::heartbeat(
            ctx(observed_at),
            seq,
            observed_at,
            None,
            "0.0.1",
            HeartbeatStatus::Alive,
            vec![],
            Attributes(BTreeMap::new()),
        )
    }

    #[test]
    fn default_capacity_is_256() {
        let p = HeartbeatProjection::default();
        assert_eq!(p.capacity(), DEFAULT_HEARTBEAT_CAPACITY);
        assert_eq!(p.capacity(), 256);
    }

    #[test]
    fn ring_honours_capacity() {
        let mut p = HeartbeatProjection::with_capacity(3);
        for i in 0..5u64 {
            p.apply(&hb(i, t0() + ChronoDuration::seconds(i as i64)))
                .unwrap();
        }
        assert_eq!(p.history.len(), 3);
        let seqs: Vec<u64> = p.history.iter().map(|p| p.value.sequence).collect();
        assert_eq!(seqs, vec![2, 3, 4]);
    }

    #[test]
    fn latest_heartbeat_returns_the_back() {
        let mut p = HeartbeatProjection::default();
        p.apply(&hb(1, t0())).unwrap();
        p.apply(&hb(2, t0() + ChronoDuration::seconds(60))).unwrap();
        let latest = p.latest_heartbeat().unwrap();
        assert_eq!(latest.value.sequence, 2);
    }

    #[test]
    fn heartbeats_since_filters_by_timestamp() {
        let mut p = HeartbeatProjection::default();
        for i in 0..4u64 {
            p.apply(&hb(i, t0() + ChronoDuration::seconds(i as i64 * 10)))
                .unwrap();
        }
        let hbs = p.heartbeats_since(t0() + ChronoDuration::seconds(15));
        let seqs: Vec<u64> = hbs.iter().map(|p| p.value.sequence).collect();
        assert_eq!(seqs, vec![2, 3]);
    }

    #[test]
    fn non_heartbeat_payload_is_ignored() {
        let mut p = HeartbeatProjection::default();
        let obs = Observation::state(
            ctx(t0()),
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
        assert!(p.history.is_empty());
    }
}
