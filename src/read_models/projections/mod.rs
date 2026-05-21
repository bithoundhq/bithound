//! Read-model projections.
//!
//! A projection consumes [`Observation`]s and maintains an in-memory
//! shape used to answer queries. The [`Projection`] trait is the common
//! ingestion surface; each variant of `ObservationPayload` has at most
//! one projection that cares about it.

pub mod metric;
pub mod state;

pub use metric::{MetricProjection, DEFAULT_METRIC_SERIES_CAPACITY};
pub use state::StateProjection;

use thiserror::Error;

use crate::observations::Observation;

/// Common ingestion surface for read-model projections.
///
/// Implementors are single-writer: the runtime applies observations to
/// projections from one consumer task, so projections do not need
/// internal synchronization.
pub trait Projection: Send + Sync + std::fmt::Debug {
    /// Fold an observation into the projection's state. Returning an
    /// error means the projection rejected the observation; the caller
    /// decides whether to log, surface, or stop.
    fn apply(&mut self, obs: &Observation) -> Result<(), ProjectionError>;
}

#[derive(Debug, Error)]
pub enum ProjectionError {
    #[error("invalid payload: {0}")]
    InvalidPayload(String),
    #[error("internal consistency: {0}")]
    InternalConsistency(String),
}
