//! Incident engine — fingerprinting, lifecycle, command handling.

use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::{
    diagnostics::types::IncidentSignalDraft,
    incidents::kinds::DraftError,
    shared::types::{ActorId, IncidentId},
};

#[derive(Debug, Clone)]
pub enum IncidentCommand {
    RecordSignal(IncidentSignalDraft),
    Acknowledge {
        id: IncidentId,
        by: ActorId,
        at: DateTime<Utc>,
    },
    Resolve {
        id: IncidentId,
        by: ActorId,
        at: DateTime<Utc>,
        reason: String,
    },
}

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("draft validation: {0}")]
    Draft(#[from] DraftError),
    #[error("command not yet implemented: {0}")]
    NotYetImplemented(&'static str),
}
