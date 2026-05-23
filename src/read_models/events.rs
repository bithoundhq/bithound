//! Per-context domain events for the read-model context.
//!
//! Per ADR-D4 — see [`crate::observations::events`] for the rationale.

use serde::{Deserialize, Serialize};

use crate::observations::Observation;

/// Things that happen at the read-model context boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReadModelEvent {
    /// A read-model projection consumed an observation.
    Applied(Observation),
}
