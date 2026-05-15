//! Types for metric observations.
use serde::{Deserialize, Serialize};

use crate::shared::types::SidecarId;

#[derive(Debug, Clone, PartialEq, Hash, Serialize, Deserialize)]
pub struct CollectorRef {
    pub name: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntegrationKind {
    BitcoinCoreRpc,
    BitcoinCoreZmq,
    LndGrpc,
    LndRest,
    Host,
}

/// Who/what produced the observation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationSource {
    pub sidecar_id: SidecarId,
    pub collector: CollectorRef,
    pub integration: IntegrationKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationOrigin {
    Collected,
    Computed,
    Imported,
    UserReported,
}
