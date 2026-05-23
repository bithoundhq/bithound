//! Types for capability observation.
use serde::{Deserialize, Serialize};

use crate::shared::parse::{parse_dotted_name, ParseDottedNameError};

/// Capability describes what Bithound can effectively
/// monitor or do with this entity.
/// # Example
///
/// ```ignore
/// CapabilityObservation {
///     capability: CapabilityName::parse("bitcoin.zmq.rawtx").unwrap(),
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

/// Canonical name for a capability (e.g. `bitcoin.zmq.rawtx`).
///
/// Constructed only through [`CapabilityName::parse`] or
/// [`CapabilityName::from_well_known`]; the inner field is private so
/// callers can't bypass validation by wrapping arbitrary strings (per
/// ADR-D2).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CapabilityName(String);

impl CapabilityName {
    pub fn parse(s: impl AsRef<str>) -> Result<Self, ParseDottedNameError> {
        parse_dotted_name(s.as_ref()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Lift a `&'static str` known to satisfy the grammar. Debug-asserts
    /// the parse rule; release builds skip the check.
    pub fn from_well_known(name: &'static str) -> Self {
        debug_assert!(
            parse_dotted_name(name).is_ok(),
            "invalid well_known capability name: {name}"
        );
        CapabilityName(name.to_string())
    }
}

impl AsRef<str> for CapabilityName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CapabilityName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for CapabilityName {
    type Error = ParseDottedNameError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl From<CapabilityName> for String {
    fn from(n: CapabilityName) -> String {
        n.0
    }
}

#[cfg(test)]
mod capability_name_tests {
    use super::*;

    #[test]
    fn parse_accepts_valid() {
        assert_eq!(
            CapabilityName::parse("bitcoin.zmq.rawtx").unwrap().as_str(),
            "bitcoin.zmq.rawtx"
        );
    }

    #[test]
    fn parse_rejects_invalid() {
        assert!(CapabilityName::parse("nodot").is_err());
        assert!(CapabilityName::parse("BadCase").is_err());
    }

    #[test]
    fn serde_revalidates() {
        let err = serde_json::from_str::<CapabilityName>("\"nodot\"").unwrap_err();
        assert!(err.to_string().contains("at least one dot"));
    }

    #[test]
    fn serde_round_trips() {
        let n = CapabilityName::parse("bitcoin.zmq.rawtx").unwrap();
        let json = serde_json::to_string(&n).unwrap();
        assert_eq!(json, "\"bitcoin.zmq.rawtx\"");
        let back: CapabilityName = serde_json::from_str(&json).unwrap();
        assert_eq!(back, n);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityStatus {
    Available,
    Unavailable,
    Degraded,
    Unknown,
}
