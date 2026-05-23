//! Types for diagnostic observations.
use serde::{Deserialize, Serialize};

use crate::observations::types::Confidence;
use crate::shared::parse::{parse_dotted_name, ParseDottedNameError};
use crate::shared::types::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosisObservation {
    pub diagnosis: DiagnosisName,
    pub summary: String,
    pub confidence: Confidence,
    pub likely_causes: Vec<String>,
    pub recommended_actions: Vec<String>,
    pub evidence: Vec<EvidenceRef>,
}

/// Canonical name for a diagnosis observation.
///
/// Constructed only through [`DiagnosisName::parse`] or
/// [`DiagnosisName::from_well_known`]; the inner field is private so
/// callers can't bypass validation by wrapping arbitrary strings (per
/// ADR-D2).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DiagnosisName(String);

impl DiagnosisName {
    pub fn parse(s: impl AsRef<str>) -> Result<Self, ParseDottedNameError> {
        parse_dotted_name(s.as_ref()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn from_well_known(name: &'static str) -> Self {
        debug_assert!(
            parse_dotted_name(name).is_ok(),
            "invalid well_known diagnosis name: {name}"
        );
        DiagnosisName(name.to_string())
    }
}

impl AsRef<str> for DiagnosisName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DiagnosisName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for DiagnosisName {
    type Error = ParseDottedNameError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl From<DiagnosisName> for String {
    fn from(n: DiagnosisName) -> String {
        n.0
    }
}
