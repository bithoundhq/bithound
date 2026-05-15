//! Types for event observations.
use serde::{Deserialize, Serialize};

/// Events represent discrete occurrences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventObservation {
    pub name: EventName,
    pub severity: EventSeverity,
    pub body: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventName(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventSeverity {
    Debug,
    Info,
    Notice,
    Warning,
    Error,
    Critical,
}
