//! Per-context domain events for the notifications context.
//!
//! Per ADR-D4 — see [`crate::observations::events`] for the rationale.

use serde::{Deserialize, Serialize};

use crate::notifications::types::NotificationAttempt;

/// Things that happen at the notifications context boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationEvent {
    /// A notification was dispatched to a target (success or terminal
    /// failure both surface here — inspect the attempt's status).
    Dispatched(NotificationAttempt),

    /// A notification was suppressed by a notifier-side rule and never
    /// sent. The attempt records the suppression reason.
    Suppressed(NotificationAttempt),
}
