//! Types for transition observations.
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransitionName(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateAtom {
    String(String),
    Bool(bool),
    U64(u64),
    I64(i64),
    F64(f64),
}
