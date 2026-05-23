//! Per-context domain events for the observation context.
//!
//! Per ADR-D4, each domain context exposes a `events.rs` module whose
//! enum names the things that cross that context's boundary. V0 does
//! not run an event bus — these enums are type-level documentation
//! used for tracing today and cloud sync later.

use serde::{Deserialize, Serialize};

use crate::observations::{Observation, ObservationBatch};

/// Things that happen at the observation context boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObservationEvent {
    /// A collector produced a batch and handed it to the consumer.
    BatchProduced(ObservationBatch),

    /// A single observation was appended to durable storage.
    ObservationAppended(Observation),
}
