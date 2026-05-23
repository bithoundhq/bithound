//! Types for inventory observations.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::shared::parse::{parse_dotted_name, ParseDottedNameError};

/// Inventory describes what the monitored entity is.
/// # Examples
/// ```ignore
/// InventoryObservation {
///     name: InventoryName::parse("bitcoin.node.inventory").unwrap(),
///     facts: BTreeMap::from([
///         ("version".into(), InventoryValue::U64(300000)),
///         ("chain".into(), InventoryValue::String("main".into())),
///         ("pruned". into(), InventoryValue::Bool(false))
///     ])
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryObservation {
    pub name: InventoryName,
    pub facts: BTreeMap<String, InventoryValue>,
}

/// Canonical name for an inventory observation.
///
/// Constructed only through [`InventoryName::parse`] or
/// [`InventoryName::from_well_known`]; the inner field is private so
/// callers can't bypass validation by wrapping arbitrary strings (per
/// ADR-D2).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct InventoryName(String);

impl InventoryName {
    pub fn parse(s: impl AsRef<str>) -> Result<Self, ParseDottedNameError> {
        parse_dotted_name(s.as_ref()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn from_well_known(name: &'static str) -> Self {
        debug_assert!(
            parse_dotted_name(name).is_ok(),
            "invalid well_known inventory name: {name}"
        );
        InventoryName(name.to_string())
    }
}

impl AsRef<str> for InventoryName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for InventoryName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for InventoryName {
    type Error = ParseDottedNameError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl From<InventoryName> for String {
    fn from(n: InventoryName) -> String {
        n.0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InventoryValue {
    String(String),
    Bool(bool),
    U64(u64),
    I64(i64),
    F64(f64),
    StringList(Vec<String>),
}
