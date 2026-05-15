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
