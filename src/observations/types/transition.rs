//! Types for transition observations.
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::shared::parse::{parse_dotted_name, ParseDottedNameError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionObservation {
    pub name: TransitionName,
    pub from: StateAtom,
    pub to: StateAtom,
    pub reason: Option<String>,
}

impl TransitionObservation {
    pub fn validate(&self) -> Result<()> {
        match (&self.from, &self.to) {
            (&StateAtom::String(_), &StateAtom::String(_)) => Ok(()),
            (&StateAtom::Bool(_), &StateAtom::Bool(_)) => Ok(()),
            (&StateAtom::U64(_), &StateAtom::U64(_)) => Ok(()),
            (&StateAtom::I64(_), &StateAtom::I64(_)) => Ok(()),
            (&StateAtom::F64(_), &StateAtom::F64(_)) => Ok(()),
            _ => Err(anyhow!("state atoms must be of the same type")),
        }
    }
}

/// Canonical name for a transition observation.
///
/// Constructed only through [`TransitionName::parse`] or
/// [`TransitionName::from_well_known`]; the inner field is private so
/// callers can't bypass validation by wrapping arbitrary strings (per
/// ADR-D2).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TransitionName(String);

impl TransitionName {
    pub fn parse(s: impl AsRef<str>) -> Result<Self, ParseDottedNameError> {
        parse_dotted_name(s.as_ref()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn from_well_known(name: &'static str) -> Self {
        debug_assert!(
            parse_dotted_name(name).is_ok(),
            "invalid well_known transition name: {name}"
        );
        TransitionName(name.to_string())
    }
}

impl AsRef<str> for TransitionName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TransitionName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for TransitionName {
    type Error = ParseDottedNameError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl From<TransitionName> for String {
    fn from(n: TransitionName) -> String {
        n.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateAtom {
    String(String),
    Bool(bool),
    U64(u64),
    I64(i64),
    F64(f64),
}
