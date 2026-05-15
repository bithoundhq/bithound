use crate::{
    observations::{HealthCheckObservation, HealthTarget},
    read_models::Projected,
    shared::types::EntityRef,
};

/// Reads the current health for a given target.
///
/// # Example:
/// ```
/// let rpc_health = health.current_health(
///     &EntityRef::BitcoinNode(node_id.clone()),
///     &HealthTarget("bitcoin.rpc".into())
/// );
/// ```
pub trait HealthReadModel: Send + Sync + std::fmt::Debug {
    fn current_health(
        &self,
        subject: &EntityRef,
        target: &HealthTarget,
    ) -> Option<Projected<HealthCheckObservation>>;
}
