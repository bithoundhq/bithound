use anyhow::Result;

use crate::diagnostics::types::{DiagnosticContext, IncidentSignalDraft};

pub trait DiagnosticRule {
    fn id(&self) -> &'static str;

    fn evaluate(&self, ctx: DiagnosticContext<'_>) -> Result<Vec<IncidentSignalDraft>>;
}
