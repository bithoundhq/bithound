use chrono::{DateTime, Utc};

use crate::{
    observations::{MetricName, MetricObservation},
    read_models::Projected,
    shared::types::EntityRef,
};

pub trait MetricReadModel: Send + Sync + std::fmt::Debug {
    fn latest_metric(
        &self,
        subject: &EntityRef,
        name: &MetricName,
    ) -> Option<Projected<MetricObservation>>;

    fn metric_samples_since(
        &self,
        subject: &EntityRef,
        name: &MetricName,
        since: DateTime<Utc>,
    ) -> Vec<Projected<MetricObservation>>;

    // Useful for stale metrics detection.
    fn unchanged_for(
        &self,
        subject: &EntityRef,
        name: &MetricName,
    ) -> Option<Vec<Projected<MetricObservation>>>;
}
