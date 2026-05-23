//! Types for health and self-check observations.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::shared::parse::{parse_dotted_name, ParseDottedNameError};

/// Health checks are active probes. The answer if the application
/// can reach and use the target at a given time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckObservation {
    pub target: HealthTargetId,
    pub status: HealthStatus,
    pub latency_ms: Option<u64>,
    pub message: Option<String>,
    pub error: Option<HealthError>,
}

/// Heartbeats describes the application liveness and
/// health itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatObservation {
    pub sequence: u64,
    pub sidecar_time: DateTime<Utc>,
    pub monotonic_uptime_ms: Option<u64>,
    pub sidecar_version: String,
    pub status: HeartbeatStatus,
    pub collector_statuses: Vec<CollectorStatus>,
}

/// The application should compute `HeartbeatStatus::Degraded` from
/// local component help. It should NOT mark itelf degraded merely
/// because the monitored node is unhealthy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeartbeatStatus {
    Alive,
    Degraded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectorStatus {
    pub collector: String,
    pub status: HealthStatus,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_error_at: Option<DateTime<Utc>>,
    pub last_error_code: Option<String>,
}

/// Canonical identifier for a health-check target (e.g. `bitcoin.rpc`).
///
/// Constructed only through [`HealthTargetId::parse`] or
/// [`HealthTargetId::from_well_known`]; the inner field is private so
/// callers can't bypass validation by wrapping arbitrary strings (per
/// ADR-D2).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct HealthTargetId(String);

impl HealthTargetId {
    pub fn parse(s: impl AsRef<str>) -> Result<Self, ParseDottedNameError> {
        parse_dotted_name(s.as_ref()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn from_well_known(name: &'static str) -> Self {
        debug_assert!(
            parse_dotted_name(name).is_ok(),
            "invalid well_known health target id: {name}"
        );
        HealthTargetId(name.to_string())
    }
}

impl AsRef<str> for HealthTargetId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for HealthTargetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for HealthTargetId {
    type Error = ParseDottedNameError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl From<HealthTargetId> for String {
    fn from(n: HealthTargetId) -> String {
        n.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Ok,
    Warning,
    Critical,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthError {
    pub code: String,
    pub message: String,
    pub retryable: Option<bool>,
}
