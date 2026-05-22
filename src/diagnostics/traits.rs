use anyhow::Result;

use crate::diagnostics::types::{DiagnosticContext, IncidentSignalDraft};

/// Diagnostic rule contract.
///
/// `Send + Sync` because the runtime stores rules behind
/// `Box<dyn DiagnosticRule>` and the consumer task moves the whole
/// `Vec` across an await boundary.
pub trait DiagnosticRule: Send + Sync {
    fn id(&self) -> &'static str;

    fn evaluate(&self, ctx: DiagnosticContext<'_>) -> Result<Vec<IncidentSignalDraft>>;
}
