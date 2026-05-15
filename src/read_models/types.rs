use crate::{
    read_models::{
        CapabilityReadModel, HealthReadModel, HeartbeatReadModel, MetricReadModel, StateReadModel,
    },
    shared::types::{EntityRef, ObservationId, Timestamp},
};

#[derive(Debug, Clone)]
pub struct Projected<T> {
    pub value: T,
    pub observation_id: ObservationId,
    pub observed_at: Timestamp,
}

#[derive(Debug)]
pub struct DiagnosticContext<'a> {
    pub now: Timestamp,
    pub subject: &'a EntityRef,

    pub state: &'a dyn StateReadModel,
    pub metrics: &'a dyn MetricReadModel,
    pub health: &'a dyn HealthReadModel,
    pub capabilities: &'a dyn CapabilityReadModel,
    pub heartbeats: Option<&'a dyn HeartbeatReadModel>,
}
