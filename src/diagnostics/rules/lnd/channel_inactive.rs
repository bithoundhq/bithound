//! `lnd.channel_inactive` — fires when an LND channel's `active`
//! field has been `false` for a sustained interval.
//!
//! Hysteresis windows differ by channel privacy: public channels get a
//! tight 5-minute window because legitimate flaps are rare; private
//! channels get 30 minutes because peer NAT-traversal causes routine
//! brief disconnects that shouldn't page anyone.
//!
//! Severity gates on the peer-online cross-reference set by the polling
//! collector. Channel inactive + peer offline is the routine case
//! (Warning / Medium). Channel inactive while peer is online is
//! suspicious (Critical / High) — typically signals a soft-failure on
//! the peer's LND like channel.db corruption or a hung wallet.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::diagnostics::traits::DiagnosticRule;
use crate::diagnostics::types::{DiagnosticContext, IncidentSignalDraft};
use crate::incidents::well_known::LND_CHANNEL_INACTIVE;
use crate::incidents::IncidentKind;
use crate::observations::state::well_known::LND_CHANNEL_DETAIL;
use crate::observations::{
    Confidence, SignalName, SignalSeverity, SignalStatus, StateName, StateObservation,
};
use crate::shared::types::EntityRef;

/// Default windows (configurable via `with_thresholds`).
const DEFAULT_PUBLIC_DEBOUNCE: Duration = Duration::from_secs(5 * 60);
const DEFAULT_PRIVATE_DEBOUNCE: Duration = Duration::from_secs(30 * 60);
const STATE_RETENTION: Duration = Duration::from_secs(60 * 60);
/// Cap for how long an `active_emitted=true` entry sticks around
/// without a fresh touch. A channel that has been gone from
/// `ListChannels` output for a day is almost certainly closed; without
/// this cap the entry pins the slot in `state` forever.
const ACTIVE_EMITTED_TTL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Default)]
struct SubjectState {
    /// When the channel was first observed inactive in a continuous
    /// stretch. `None` while the channel is active.
    first_inactive_at: Option<Instant>,
    /// Whether we already emitted an Active draft for the current
    /// inactivity window.
    active_emitted: bool,
    last_touched_at: Option<Instant>,
}

#[derive(Debug)]
pub struct LndChannelInactiveRule {
    public_debounce: Duration,
    private_debounce: Duration,
    state: Mutex<HashMap<EntityRef, SubjectState>>,
}

impl Default for LndChannelInactiveRule {
    fn default() -> Self {
        Self::with_thresholds(DEFAULT_PUBLIC_DEBOUNCE, DEFAULT_PRIVATE_DEBOUNCE)
    }
}

impl LndChannelInactiveRule {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_thresholds(public_debounce: Duration, private_debounce: Duration) -> Self {
        Self {
            public_debounce,
            private_debounce,
            state: Mutex::new(HashMap::new()),
        }
    }
}

impl DiagnosticRule for LndChannelInactiveRule {
    fn id(&self) -> &'static str {
        "lnd.channel_inactive"
    }

    fn evaluate(&self, ctx: DiagnosticContext<'_>) -> Result<Vec<IncidentSignalDraft>> {
        // Only meaningful for LndChannel subjects.
        if !matches!(ctx.subject, EntityRef::LndChannel { .. }) {
            return Ok(vec![]);
        }

        // Look up the latest LndChannel state. If we have no
        // observation, there's nothing to evaluate (the polling
        // collector hasn't reported on this channel yet).
        let channel_state =
            match ctx
                .state
                .latest_state(ctx.subject, &StateName::from_well_known(LND_CHANNEL_DETAIL))
            {
                Some(snapshot) => match snapshot.value {
                    StateObservation::LndChannel(c) => c,
                    _ => return Ok(vec![]),
                },
                None => return Ok(vec![]),
            };

        let mut guard = lock_state(&self.state);
        prune_stale(&mut guard, ctx.monotonic_now);
        if !guard.contains_key(ctx.subject) {
            guard.insert(ctx.subject.clone(), SubjectState::default());
        }
        let entry = guard
            .get_mut(ctx.subject)
            .expect("inserted above if missing");
        entry.last_touched_at = Some(ctx.monotonic_now);

        let mut drafts: Vec<IncidentSignalDraft> = Vec::new();

        if channel_state.active {
            // Channel back to active: clear the firing-window counter
            // and emit Cleared if we previously opened.
            entry.first_inactive_at = None;
            if entry.active_emitted {
                entry.active_emitted = false;
                drafts.push(build_draft(
                    ctx.subject,
                    SignalStatus::Cleared,
                    severity_and_confidence(channel_state.peer_online),
                ));
            }
        } else {
            let first = *entry.first_inactive_at.get_or_insert(ctx.monotonic_now);
            let held = ctx.monotonic_now.saturating_duration_since(first);
            let debounce = if channel_state.private {
                self.private_debounce
            } else {
                self.public_debounce
            };

            if !entry.active_emitted && held >= debounce {
                entry.active_emitted = true;
                drafts.push(build_draft(
                    ctx.subject,
                    SignalStatus::Active,
                    severity_and_confidence(channel_state.peer_online),
                ));
            }
        }

        Ok(drafts)
    }
}

/// `(severity, confidence)` per the peer-online cross-reference:
/// peer-offline channel inactivity is routine; peer-online channel
/// inactivity is suspicious.
fn severity_and_confidence(peer_online: Option<bool>) -> (SignalSeverity, Confidence) {
    match peer_online {
        Some(true) => (SignalSeverity::Critical, Confidence::High),
        // peer offline -> routine cause; peer status unknown -> stay
        // conservative.
        Some(false) | None => (SignalSeverity::Warning, Confidence::Medium),
    }
}

fn build_draft(
    subject: &EntityRef,
    status: SignalStatus,
    (severity, confidence): (SignalSeverity, Confidence),
) -> IncidentSignalDraft {
    let kind = IncidentKind::from_well_known(LND_CHANNEL_INACTIVE);
    IncidentSignalDraft {
        subject: subject.clone(),
        signal: SignalName::for_incident_kind(&kind),
        kind,
        dimension: None,
        severity,
        status,
        confidence,
        evidence: vec![],
    }
}

fn lock_state(
    mutex: &Mutex<HashMap<EntityRef, SubjectState>>,
) -> std::sync::MutexGuard<'_, HashMap<EntityRef, SubjectState>> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn prune_stale(state: &mut HashMap<EntityRef, SubjectState>, now: Instant) {
    state.retain(|_, entry| {
        let retention = if entry.active_emitted {
            ACTIVE_EMITTED_TTL
        } else {
            STATE_RETENTION
        };
        match entry.last_touched_at {
            Some(ts) => now.saturating_duration_since(ts) < retention,
            None => true,
        }
    });
}
