//! Types for capability observation.
use serde::{Deserialize, Serialize};

/// Capability describes what Bithound can effectively
/// monitor or do with this entity.
/// # Example
///
/// ```
/// CapabilityObservation {
///     capability: CapabilityName("bitcoin.zmq.rawtx".into()),
///     status: CapabilityStatus::Unavailable,
///     reason: Some("rawtx publisher not reported by getzmqnotifications".into())
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityObservation {
    pub capability: CapabilityName,
    pub status: CapabilityStatus,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CapabilityName(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityStatus {
    Available,
    Unavailable,
    Degraded,
    Unknown,
}
