//! Types for incident signal observations.
//! Take note that these are not incidents per se.
use serde::{Deserialize, Serialize};

use crate::incidents::IncidentKind;
use crate::shared::parse::{parse_dotted_name, ParseDottedNameError};
use crate::shared::types::*;

/// Incident signals are derived detection primitives.
/// They are not to be confused with full incidents themselves.
///
/// `incident_kind` records which incident kind this signal contributes
/// to, so the read-model layer can answer `active_signals_for_incident_kind`
/// without re-deriving the mapping. The engine populates it from the
/// originating `IncidentSignalDraft::kind`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentSignalObservation {
    pub signal: SignalName,
    pub incident_kind: IncidentKind,
    pub severity: SignalSeverity,
    pub status: SignalStatus,
    pub confidence: Confidence,
    pub evidence: Vec<EvidenceRef>,
}

/// Canonical name for an incident signal (e.g. `bitcoin.tip_lag.signal`).
///
/// Constructed only through [`SignalName::parse`],
/// [`SignalName::for_incident_kind`], or
/// [`SignalName::from_well_known`]; the inner field is private so
/// callers can't bypass validation by wrapping arbitrary strings (per
/// ADR-D2).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SignalName(String);

impl SignalName {
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
            "invalid well_known signal name: {name}"
        );
        SignalName(name.to_string())
    }

    /// Canonical signal name derived from an [`IncidentKind`]: the kind's
    /// dotted name with a `.signal` suffix.
    ///
    /// Centralizing the format here keeps the rule-to-signal-name mapping
    /// from drifting across the codebase. Tests and rules construct
    /// signal names via this helper rather than reformatting the suffix
    /// by hand. The result is dotted by construction (the suffix adds a
    /// segment) so the parse rule holds.
    pub fn for_incident_kind(kind: &crate::incidents::IncidentKind) -> Self {
        SignalName(format!("{}.signal", kind.as_str()))
    }
}

impl AsRef<str> for SignalName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SignalName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for SignalName {
    type Error = ParseDottedNameError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl From<SignalName> for String {
    fn from(n: SignalName) -> String {
        n.0
    }
}

#[cfg(test)]
mod signal_name_tests {
    use super::*;

    #[test]
    fn parse_accepts_valid() {
        assert_eq!(
            SignalName::parse("bitcoin.tip_lag.signal")
                .unwrap()
                .as_str(),
            "bitcoin.tip_lag.signal"
        );
    }

    #[test]
    fn parse_rejects_invalid() {
        assert!(SignalName::parse("signal").is_err());
        assert!(SignalName::parse("BadCase").is_err());
    }

    #[test]
    fn serde_revalidates() {
        let err = serde_json::from_str::<SignalName>("\"signal\"").unwrap_err();
        assert!(err.to_string().contains("at least one dot"));
    }

    #[test]
    fn serde_round_trips() {
        let n = SignalName::parse("bitcoin.tip_lag.signal").unwrap();
        let json = serde_json::to_string(&n).unwrap();
        assert_eq!(json, "\"bitcoin.tip_lag.signal\"");
        let back: SignalName = serde_json::from_str(&json).unwrap();
        assert_eq!(back, n);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalStatus {
    Active,
    Cleared,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Confidence {
    Low,
    Medium,
    High,
}
