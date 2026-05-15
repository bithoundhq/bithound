use crate::{
    incidents::IncidentKind,
    observations::{IncidentSignalObservation, SignalName},
    read_models::Projected,
    shared::types::EntityRef,
};

mod capability;
mod health;
mod heartbeat;
mod incident_signal;
mod metric;
mod state;

pub use capability::CapabilityReadModel;
pub use health::HealthReadModel;
pub use heartbeat::HeartbeatReadModel;
pub use incident_signal::IncidentSignalReadModel;
pub use metric::MetricReadModel;
pub use state::StateReadModel;
