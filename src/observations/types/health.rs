//! Types for health and self-check observations.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HealthTargetId(pub String);

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
