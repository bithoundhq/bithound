//! Metric ring projection.
//!
//! Stores a bounded ring of `Projected<MetricObservation>` per
//! `(subject, metric name)`. Each ring has a configurable per-series
//! capacity (default 1000). Inserts always go to the back; once
//! capacity is exceeded the oldest sample is evicted from the front.

use std::collections::{HashMap, VecDeque};

use chrono::{DateTime, Utc};

use crate::{
    observations::{MetricName, MetricObservation, Observation, ObservationPayload},
    read_models::{Projected, Projection, ProjectionError},
    shared::types::EntityRef,
};

pub const DEFAULT_METRIC_SERIES_CAPACITY: usize = 1000;

#[derive(Debug)]
pub struct MetricProjection {
    capacity: usize,
    by_key: HashMap<(EntityRef, MetricName), VecDeque<Projected<MetricObservation>>>,
}

impl Default for MetricProjection {
    fn default() -> Self {
        Self::with_capacity(DEFAULT_METRIC_SERIES_CAPACITY)
    }
}

impl MetricProjection {
    /// Per-series capacity. Each `(subject, name)` ring holds at most
    /// `capacity` samples; older samples are evicted FIFO.
    ///
    /// Panics if `capacity` is zero — a zero-capacity ring can never
    /// retain a sample and is almost certainly a misconfiguration.
    pub fn with_capacity(capacity: usize) -> Self {
        assert!(capacity > 0, "MetricProjection capacity must be > 0");
        Self {
            capacity,
            by_key: HashMap::new(),
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The most recent sample for the series, or `None`.
    pub fn latest_metric(
        &self,
        subject: &EntityRef,
        name: &MetricName,
    ) -> Option<Projected<MetricObservation>> {
        self.by_key
            .get(&(subject.clone(), name.clone()))
            .and_then(|q| q.back().cloned())
    }

    /// Samples strictly newer than `since`, oldest-first.
    pub fn metric_samples_since(
        &self,
        subject: &EntityRef,
        name: &MetricName,
        since: DateTime<Utc>,
    ) -> Vec<Projected<MetricObservation>> {
        let Some(q) = self.by_key.get(&(subject.clone(), name.clone())) else {
            return Vec::new();
        };
        q.iter()
            .filter(|p| p.observed_at > since)
            .cloned()
            .collect()
    }

    /// Returns the trailing run of samples whose `MetricValue` equals
    /// the latest sample's value, oldest-first. The latest sample is
    /// always included if the series is non-empty.
    pub fn unchanged_for(
        &self,
        subject: &EntityRef,
        name: &MetricName,
    ) -> Option<Vec<Projected<MetricObservation>>> {
        let q = self.by_key.get(&(subject.clone(), name.clone()))?;
        let latest = q.back()?;
        let latest_value = &latest.value.value;

        let run: Vec<_> = q
            .iter()
            .rev()
            .take_while(|p| &p.value.value == latest_value)
            .cloned()
            .collect();
        let mut run = run;
        run.reverse();
        Some(run)
    }
}

impl Projection for MetricProjection {
    fn apply(&mut self, obs: &Observation) -> Result<(), ProjectionError> {
        let metric = match &obs.payload {
            ObservationPayload::Metric(m) => m,
            _ => return Ok(()),
        };

        let key = (obs.subject.clone(), metric.name.clone());
        let q = self.by_key.entry(key).or_default();
        q.push_back(Projected {
            value: metric.clone(),
            observation_id: obs.id.clone(),
            observed_at: obs.observed_at,
        });
        while q.len() > self.capacity {
            q.pop_front();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::{CollectorRef, IntegrationKind};
    use crate::observations::{
        Attributes, MetricKind, MetricValue, NumericValue, ObservationContext, ObservationOrigin,
        ObservationSource, Unit,
    };
    use crate::shared::types::{BitcoinNodeId, CollectorId, SidecarId};
    use chrono::{Duration as ChronoDuration, TimeZone};
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

    fn metric_obs(
        subject: EntityRef,
        name: &str,
        value: u64,
        observed_at: chrono::DateTime<Utc>,
    ) -> Observation {
        Observation::metric(
            ctx(subject, observed_at),
            name,
            MetricKind::Gauge,
            MetricValue::Numeric(NumericValue::U64(value)),
            Unit::Count,
            Attributes(BTreeMap::new()),
        )
    }

    fn btc() -> EntityRef {
        EntityRef::BitcoinNode(BitcoinNodeId("alice".into()))
    }

    fn name() -> MetricName {
        MetricName("peer_count".into())
    }

    #[test]
    fn default_capacity_is_1000() {
        let p = MetricProjection::default();
        assert_eq!(p.capacity(), DEFAULT_METRIC_SERIES_CAPACITY);
        assert_eq!(p.capacity(), 1000);
    }

    #[test]
    fn with_capacity_sets_the_per_series_limit() {
        let p = MetricProjection::with_capacity(5);
        assert_eq!(p.capacity(), 5);
    }

    #[test]
    #[should_panic(expected = "capacity must be > 0")]
    fn with_zero_capacity_panics() {
        MetricProjection::with_capacity(0);
    }

    #[test]
    fn fifo_eviction_at_capacity() {
        let mut p = MetricProjection::with_capacity(3);
        for i in 0..5 {
            p.apply(&metric_obs(
                btc(),
                "peer_count",
                i as u64,
                t0() + ChronoDuration::seconds(i),
            ))
            .unwrap();
        }
        let q = p.by_key.get(&(btc(), name())).unwrap();
        assert_eq!(q.len(), 3);
        let values: Vec<u64> = q
            .iter()
            .map(|p| match &p.value.value {
                MetricValue::Numeric(NumericValue::U64(v)) => *v,
                _ => panic!(),
            })
            .collect();
        assert_eq!(values, vec![2, 3, 4]);
    }

    #[test]
    fn latest_metric_returns_the_back_of_the_ring() {
        let mut p = MetricProjection::with_capacity(10);
        for i in 1..=3u64 {
            p.apply(&metric_obs(
                btc(),
                "peer_count",
                i * 10,
                t0() + ChronoDuration::seconds(i as i64),
            ))
            .unwrap();
        }
        let latest = p.latest_metric(&btc(), &name()).unwrap();
        match latest.value.value {
            MetricValue::Numeric(NumericValue::U64(v)) => assert_eq!(v, 30),
            _ => panic!(),
        }
    }

    #[test]
    fn metric_samples_since_filters_by_timestamp_and_is_oldest_first() {
        let mut p = MetricProjection::with_capacity(10);
        for i in 0..5u64 {
            p.apply(&metric_obs(
                btc(),
                "peer_count",
                i,
                t0() + ChronoDuration::seconds(i as i64 * 10),
            ))
            .unwrap();
        }
        // since t0+15 → take samples at t0+20, t0+30, t0+40 (values 2,3,4)
        let samples = p.metric_samples_since(&btc(), &name(), t0() + ChronoDuration::seconds(15));
        let values: Vec<u64> = samples
            .iter()
            .map(|p| match p.value.value {
                MetricValue::Numeric(NumericValue::U64(v)) => v,
                _ => panic!(),
            })
            .collect();
        assert_eq!(values, vec![2, 3, 4]);
    }

    #[test]
    fn unchanged_for_returns_at_least_the_latest_sample() {
        let mut p = MetricProjection::with_capacity(10);
        p.apply(&metric_obs(btc(), "peer_count", 42, t0())).unwrap();
        let run = p.unchanged_for(&btc(), &name()).unwrap();
        assert_eq!(run.len(), 1);
    }

    #[test]
    fn unchanged_for_returns_trailing_run_of_equal_values() {
        let mut p = MetricProjection::with_capacity(10);
        let series = [1u64, 2, 2, 5, 5, 5];
        for (i, v) in series.iter().enumerate() {
            p.apply(&metric_obs(
                btc(),
                "peer_count",
                *v,
                t0() + ChronoDuration::seconds(i as i64),
            ))
            .unwrap();
        }
        let run = p.unchanged_for(&btc(), &name()).unwrap();
        let values: Vec<u64> = run
            .iter()
            .map(|p| match p.value.value {
                MetricValue::Numeric(NumericValue::U64(v)) => v,
                _ => panic!(),
            })
            .collect();
        assert_eq!(values, vec![5, 5, 5]);
    }

    #[test]
    fn unchanged_for_returns_none_when_series_does_not_exist() {
        let p = MetricProjection::with_capacity(10);
        assert!(p.unchanged_for(&btc(), &name()).is_none());
    }

    #[test]
    fn non_metric_payload_is_ignored() {
        let mut p = MetricProjection::with_capacity(10);
        let obs = Observation::state(
            ctx(btc(), t0()),
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

    /// Capacity invariant — after ≫ capacity inserts, no series exceeds capacity.
    #[test]
    fn capacity_invariant_holds_after_many_inserts() {
        let cap = 50;
        let mut p = MetricProjection::with_capacity(cap);
        for i in 0..(cap * 10) as u64 {
            p.apply(&metric_obs(
                btc(),
                "peer_count",
                i,
                t0() + ChronoDuration::seconds(i as i64),
            ))
            .unwrap();
        }
        let q = p.by_key.get(&(btc(), name())).unwrap();
        assert_eq!(q.len(), cap);
        // Front is the oldest survivor and back is the newest insert.
        let front = match q.front().unwrap().value.value {
            MetricValue::Numeric(NumericValue::U64(v)) => v,
            _ => panic!(),
        };
        let back = match q.back().unwrap().value.value {
            MetricValue::Numeric(NumericValue::U64(v)) => v,
            _ => panic!(),
        };
        assert_eq!(back - front, (cap as u64) - 1);
    }
}
