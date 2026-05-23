//! Types describing an observation's provenance.
use serde::{Deserialize, Serialize};

use crate::{collectors::CollectorRef, shared::types::SidecarId};

/// Who/what produced the observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationSource {
    pub sidecar_id: SidecarId,
    pub collector: CollectorRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationOrigin {
    Collected,
    Computed,
    Imported,
    UserReported,
}

impl ObservationOrigin {
    /// Stable wire-format string. Use this when rendering origins into
    /// API responses, logs, or storage so the operator-facing value is
    /// intentional, not coupled to Rust's `Debug` derive.
    pub fn as_str(&self) -> &'static str {
        match self {
            ObservationOrigin::Collected => "Collected",
            ObservationOrigin::Computed => "Computed",
            ObservationOrigin::Imported => "Imported",
            ObservationOrigin::UserReported => "UserReported",
        }
    }
}
