//! Types for inventory observations.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Inventory describes what the monitored entity is.
/// # Examples
/// ```
/// InventoryObservation {
///     name: InventoryName("bitcoin.node.inventory".into()),
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InventoryName(pub String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InventoryValue {
    String(String),
    Bool(bool),
    U64(u64),
    I64(i64),
    F64(f64),
    StringList(Vec<String>),
}
