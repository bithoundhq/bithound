//! Incident engine — fingerprinting, lifecycle, command handling.

use chrono::{DateTime, Utc};

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

#[derive(Debug)]
pub enum EngineError {
    Draft(DraftError),
    NotYetImplemented(&'static str),
}
