use chrono::{DateTime, Utc};

use crate::{observations::HeartbeatObservation, read_models::Projected};

// This is most for future cloud-side diagnostics, but it can also be useful locally.
pub trait HeartbeatReadModel: Send + Sync + std::fmt::Debug {
    fn latest_heartbeat(&self) -> Option<Projected<HeartbeatObservation>>;
    fn heartbeats_since(&self, since: DateTime<Utc>) -> Vec<Projected<HeartbeatObservation>>;
}
