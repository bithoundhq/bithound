//! Notification dispatch worker.
//!
//! Per ADR-N2: the consumer never calls senders directly. Lifecycle
//! events surface here as `NotificationDispatch` messages, the
//! worker drives each target's sender, and the matching
//! `NotificationAttempt` row is flipped from `Pending` to a terminal
//! status. If the worker dies mid-dispatch the row stays `Pending` —
//! that's the audit-trail guarantee.

use crate::incidents::IncidentLifecycleEvent;
use crate::notifications::types::{
    NotificationAttemptId, NotificationMessage, NotificationTarget,
};

/// Message the consumer sends to the worker for one lifecycle event.
/// Carries the resolved targets (including their `SecretString`
/// credentials) so the worker can call the matching sender. The
/// targets are paired with the Pending `NotificationAttemptId` rows
/// the consumer already INSERTed, so the worker knows which row to
/// UPDATE on completion.
#[derive(Debug, Clone)]
pub struct NotificationDispatch {
    pub event: IncidentLifecycleEvent,
    pub message: NotificationMessage,
    pub attempts: Vec<NotificationAttemptId>,
    pub targets: Vec<(NotificationAttemptId, NotificationTarget)>,
}
