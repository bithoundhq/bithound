use crate::{observations::HeartbeatObservation, read_models::Projected, shared::types::Timestamp};

// This is most for future cloud-side diagnostics, but it can also be useful locally.
pub trait HeartbeatReadModel: Send + Sync + std::fmt::Debug {
    fn latest_heartbeat(&self) -> Option<Projected<HeartbeatObservation>>;
    fn heartbeats_since(&self, since: Timestamp) -> Vec<Projected<HeartbeatObservation>>;
}
