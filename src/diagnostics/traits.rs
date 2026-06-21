use anyhow::Result;

use crate::diagnostics::types::{DiagnosticContext, IncidentSignalDraft};

/// Diagnostic rule contract.
///
/// `Send` (no `Sync`) because the runtime stores rules behind
/// `Box<dyn DiagnosticRule>` inside the single consumer task (per
/// ADR-S1) — the consumer task owns `&mut` on the rule slice and
/// calls `evaluate` sequentially per batch. Rules carry their own
/// hysteresis state directly on `&mut self`; no interior mutability
/// (`Mutex`, `RefCell`) is needed.
pub trait DiagnosticRule: Send {
    fn id(&self) -> &'static str;

    fn evaluate(&mut self, ctx: DiagnosticContext<'_>) -> Result<Vec<IncidentSignalDraft>>;
}
