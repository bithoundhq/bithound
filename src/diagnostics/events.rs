//! Per-context domain events for the diagnostics context.
//!
//! Per ADR-D4 — see [`crate::observations::events`] for the rationale.

use serde::{Deserialize, Serialize};

use crate::diagnostics::types::IncidentSignalDraft;

/// Things that happen at the diagnostics context boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiagnosticEvent {
    /// A rule produced a draft for the engine to validate.
    DraftEmitted {
        rule_id: String,
        draft: IncidentSignalDraft,
    },

    /// A rule's `evaluate` returned `Err`; the consumer logs and skips it.
    RuleFailed { rule_id: String, error: String },
}
