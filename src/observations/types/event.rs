//! Types for event observations.
use serde::{Deserialize, Serialize};

use crate::shared::parse::{parse_dotted_name, ParseDottedNameError};

/// Events represent discrete occurrences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventObservation {
    pub name: EventName,
    pub severity: EventSeverity,
    pub body: Option<String>,
}

/// Canonical name for an event (e.g. `sidecar.collector.run_started`).
///
/// Constructed only through [`EventName::parse`] or
/// [`EventName::from_well_known`]; the inner field is private so
/// callers can't bypass validation by wrapping arbitrary strings (per
/// ADR-D2).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct EventName(String);

impl EventName {
    pub fn parse(s: impl AsRef<str>) -> Result<Self, ParseDottedNameError> {
        parse_dotted_name(s.as_ref()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn from_well_known(name: &'static str) -> Self {
        debug_assert!(
            parse_dotted_name(name).is_ok(),
            "invalid well_known event name: {name}"
        );
        EventName(name.to_string())
    }
}

impl AsRef<str> for EventName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for EventName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for EventName {
    type Error = ParseDottedNameError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl From<EventName> for String {
    fn from(n: EventName) -> String {
        n.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventSeverity {
    Debug,
    Info,
    Notice,
    Warning,
    Error,
    Critical,
}
