use crate::{
    observations::{CapabilityName, CapabilityObservation},
    read_models::Projected,
    shared::types::EntityRef,
};

// Useful for rules like bitcoin.zmq_not_configured
pub trait CapabilityReadModel: Send + Sync + std::fmt::Debug {
    fn current_capability(
        &self,
        subject: &EntityRef,
        capability: &CapabilityName,
    ) -> Option<Projected<CapabilityObservation>>;
    fn capabilities_for(&self, subject: &EntityRef) -> Vec<Projected<CapabilityObservation>>;
}
