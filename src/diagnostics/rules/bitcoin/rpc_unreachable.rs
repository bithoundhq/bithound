//! `bitcoin.rpc_unreachable` — fires when all four Bitcoin Core RPC
//! health targets are simultaneously `Critical` for a sustained interval.
//!
//! The four targets — `getblockchaininfo`, `getmempoolinfo`,
//! `getnetworkinfo`, `getpeerinfo` — together cover the surface of the
//! RPC that bithound's collectors actually call. If every one of them
//! is failing, the operator can't query the node at all; that's the
//! single most important condition to surface.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::collectors::bitcoin_core::rpc::HEALTH_TARGETS as RPC_TARGETS;
use crate::diagnostics::traits::DiagnosticRule;
use crate::diagnostics::types::{DiagnosticContext, IncidentSignalDraft};
use crate::incidents::well_known::BITCOIN_RPC_UNREACHABLE;
use crate::incidents::IncidentKind;
use crate::observations::{
    Confidence, HealthStatus, HealthTargetId, SignalName, SignalSeverity, SignalStatus,
};
use crate::shared::types::EntityRef;

/// How long all four targets must remain `Critical` before we open
/// an incident.
const DEFAULT_DEBOUNCE: Duration = Duration::from_secs(60);

/// Idle subjects (no firing condition, no open incident) get dropped from
/// the in-memory state map after this long without being touched, so a
/// long-lived sidecar that sees subjects come and go doesn't grow the
/// map without bound.
const STATE_RETENTION: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Default)]
struct SubjectState {
    /// When the "all four Critical" condition first held continuously
    /// for this subject. `None` while at least one target is non-Critical.
    ///
    /// Stored as a monotonic [`Instant`] so the debounce window is
    /// immune to wall-clock skew (NTP corrections, VM suspend/resume).
    first_all_critical_at: Option<Instant>,
    /// Whether we have already emitted an `Active` draft for the
    /// current outage. Used to avoid re-emitting on every tick once
    /// the engine has opened the incident.
    active_emitted: bool,
    /// Last evaluation `monotonic_now` for this subject. Drives pruning
    /// of stale idle entries.
    last_touched_at: Option<Instant>,
}

/// `BitcoinRpcUnreachableRule` implements `DiagnosticRule` for the
/// `bitcoin.rpc_unreachable` kind. Stateful: tracks per-subject
/// "when did all-four-Critical begin" so the rule can debounce a brief
/// outage without producing an incident.
#[derive(Debug)]
pub struct BitcoinRpcUnreachableRule {
    debounce: Duration,
    state: Mutex<HashMap<EntityRef, SubjectState>>,
}

impl Default for BitcoinRpcUnreachableRule {
    fn default() -> Self {
        Self::with_debounce(DEFAULT_DEBOUNCE)
    }
}

impl BitcoinRpcUnreachableRule {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_debounce(debounce: Duration) -> Self {
        Self {
            debounce,
            state: Mutex::new(HashMap::new()),
        }
    }

    fn all_four_critical(&self, ctx: &DiagnosticContext<'_>) -> bool {
        RPC_TARGETS.iter().all(|name| {
            matches!(
                ctx.health
                    .current_health(ctx.subject, &HealthTargetId::from_well_known(name))
                    .map(|p| p.value.status),
                Some(HealthStatus::Critical)
            )
        })
    }
}

impl DiagnosticRule for BitcoinRpcUnreachableRule {
    fn id(&self) -> &'static str {
        "bitcoin.rpc_unreachable"
    }

    fn evaluate(&self, ctx: DiagnosticContext<'_>) -> Result<Vec<IncidentSignalDraft>> {
        // Only meaningful for Bitcoin node subjects.
        //
        // Note on "silent" inputs: a target with no health observation
        // at all is treated as not-Critical (the `Some(Critical)` match
        // below fails), so a fully-dead collector that never emits will
        // not raise an incident here. That's intentionally conservative
        // — a missing-collector incident is the heartbeat rule's job in
        // V0.1; this rule only speaks to what the RPC layer is actually
        // reporting.
        if !matches!(ctx.subject, EntityRef::BitcoinNode(_)) {
            return Ok(vec![]);
        }

        let all_critical = self.all_four_critical(&ctx);

        let mut guard = lock_state(&self.state);
        prune_stale(&mut guard, ctx.monotonic_now);
        // Look up first so we only clone the subject key when we
        // actually have to insert. Lets the hot "subject already known"
        // path skip the allocation entirely.
        if !guard.contains_key(ctx.subject) {
            guard.insert(ctx.subject.clone(), SubjectState::default());
        }
        let entry = guard
            .get_mut(ctx.subject)
            .expect("inserted above if missing");
        entry.last_touched_at = Some(ctx.monotonic_now);

        let mut drafts: Vec<IncidentSignalDraft> = Vec::new();

        if all_critical {
            let first = *entry.first_all_critical_at.get_or_insert(ctx.monotonic_now);
            // `Instant::saturating_duration_since` returns zero on a
            // backwards-going argument; with a monotonic clock this is
            // already impossible, but the saturating form is the
            // panic-free contract.
            let held = ctx.monotonic_now.saturating_duration_since(first);

            if !entry.active_emitted && held >= self.debounce {
                entry.active_emitted = true;
                drafts.push(build_draft(ctx.subject, SignalStatus::Active));
            }
        } else {
            entry.first_all_critical_at = None;
            if entry.active_emitted {
                entry.active_emitted = false;
                drafts.push(build_draft(ctx.subject, SignalStatus::Cleared));
            }
        }

        Ok(drafts)
    }
}

/// Acquire the rule's state lock, recovering through poisoning.
///
/// A panic inside one `evaluate` call would leave the mutex poisoned and
/// every subsequent call would re-panic, which the consumer task's
/// `catch_unwind` wrapper would just log over and over. Better to take
/// the inner data and keep working — the next evaluation rebuilds the
/// firing/clearing counters from observed state anyway.
fn lock_state(
    mutex: &Mutex<HashMap<EntityRef, SubjectState>>,
) -> std::sync::MutexGuard<'_, HashMap<EntityRef, SubjectState>> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Drop entries that haven't been touched in [`STATE_RETENTION`] and
/// have no active incident outstanding. Bounds memory growth for
/// long-running sidecars that see subjects come and go.
fn prune_stale(state: &mut HashMap<EntityRef, SubjectState>, now: Instant) {
    state.retain(|_, entry| {
        if entry.active_emitted {
            return true;
        }
        match entry.last_touched_at {
            Some(ts) => now.saturating_duration_since(ts) < STATE_RETENTION,
            None => true,
        }
    });
}

fn build_draft(subject: &EntityRef, status: SignalStatus) -> IncidentSignalDraft {
    let kind = IncidentKind::from_well_known(BITCOIN_RPC_UNREACHABLE);
    IncidentSignalDraft {
        subject: subject.clone(),
        signal: SignalName::for_incident_kind(&kind),
        kind,
        dimension: None,
        severity: SignalSeverity::Critical,
        status,
        confidence: Confidence::High,
        evidence: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::rules::bitcoin::test_support::FakeReadModels;
    use crate::shared::types::BitcoinNodeId;
    use chrono::{DateTime, TimeZone, Utc};

    fn t0() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 21, 12, 0, 0).unwrap()
    }

    fn node() -> EntityRef {
        EntityRef::BitcoinNode(BitcoinNodeId("alice".into()))
    }

    fn ctx<'a>(
        rm: &'a FakeReadModels,
        subject: &'a EntityRef,
        now: DateTime<Utc>,
        monotonic_now: Instant,
    ) -> DiagnosticContext<'a> {
        DiagnosticContext {
            now,
            monotonic_now,
            subject,
            state: rm,
            metrics: rm,
            health: rm,
            capabilities: rm,
            heartbeats: rm,
            signals: rm,
        }
    }

    fn set_all_critical(rm: &mut FakeReadModels, observed_at: DateTime<Utc>) {
        for t in RPC_TARGETS {
            rm.set_health(&node(), t, HealthStatus::Critical, observed_at);
        }
    }

    #[test]
    fn id_is_kind_name() {
        let rule = BitcoinRpcUnreachableRule::new();
        assert_eq!(rule.id(), "bitcoin.rpc_unreachable");
    }

    /// Build a monotonic-clock offset from a base `Instant`. Tests bind
    /// `i0` once and step forward in fixed durations; `Instant` cannot
    /// be constructed at an arbitrary "absolute" point, so we anchor on
    /// `Instant::now()` and offset from there.
    fn step(base: Instant, secs: u64) -> Instant {
        base + Duration::from_secs(secs)
    }

    #[test]
    fn all_four_critical_for_sixty_seconds_emits_active() {
        let mut rm = FakeReadModels::default();
        set_all_critical(&mut rm, t0());

        let rule = BitcoinRpcUnreachableRule::new();
        let subject = node();
        let i0 = Instant::now();

        // First tick: condition just started, debounce not satisfied.
        let drafts = rule
            .evaluate(ctx(&rm, &subject, t0(), i0))
            .expect("evaluate");
        assert!(drafts.is_empty(), "first tick within debounce → no draft");

        // 60 seconds later, condition still holds → Active draft.
        let drafts = rule
            .evaluate(ctx(
                &rm,
                &subject,
                t0() + chrono::Duration::seconds(60),
                step(i0, 60),
            ))
            .expect("evaluate");
        assert_eq!(drafts.len(), 1);
        let d = &drafts[0];
        assert_eq!(d.status, SignalStatus::Active);
        assert_eq!(
            d.kind,
            IncidentKind::from_well_known(BITCOIN_RPC_UNREACHABLE)
        );
        assert_eq!(d.severity, SignalSeverity::Critical);
        assert_eq!(d.confidence, Confidence::High);
        assert_eq!(d.subject, node());
        assert_eq!(d.dimension, None);
    }

    #[test]
    fn brief_outage_under_sixty_seconds_emits_nothing() {
        let mut rm = FakeReadModels::default();
        set_all_critical(&mut rm, t0());

        let rule = BitcoinRpcUnreachableRule::new();
        let subject = node();
        let i0 = Instant::now();

        assert!(rule
            .evaluate(ctx(&rm, &subject, t0(), i0))
            .unwrap()
            .is_empty());

        // 30s later one target recovers — well before the 60s window.
        rm.set_health(
            &node(),
            "bitcoin.rpc.getmempoolinfo",
            HealthStatus::Ok,
            t0() + chrono::Duration::seconds(30),
        );
        let drafts = rule
            .evaluate(ctx(
                &rm,
                &subject,
                t0() + chrono::Duration::seconds(30),
                step(i0, 30),
            ))
            .unwrap();
        assert!(drafts.is_empty(), "brief outage → no draft");
    }

    #[test]
    fn partial_recovery_after_active_emits_cleared() {
        let mut rm = FakeReadModels::default();
        set_all_critical(&mut rm, t0());

        let rule = BitcoinRpcUnreachableRule::new();
        let subject = node();
        let i0 = Instant::now();

        // Drive through the debounce so the rule emits Active.
        let _ = rule.evaluate(ctx(&rm, &subject, t0(), i0)).unwrap();
        let active = rule
            .evaluate(ctx(
                &rm,
                &subject,
                t0() + chrono::Duration::seconds(60),
                step(i0, 60),
            ))
            .unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].status, SignalStatus::Active);

        // One target recovers — partial recovery is enough to clear.
        rm.set_health(
            &node(),
            "bitcoin.rpc.getnetworkinfo",
            HealthStatus::Ok,
            t0() + chrono::Duration::seconds(90),
        );
        let cleared = rule
            .evaluate(ctx(
                &rm,
                &subject,
                t0() + chrono::Duration::seconds(90),
                step(i0, 90),
            ))
            .unwrap();
        assert_eq!(cleared.len(), 1);
        assert_eq!(cleared[0].status, SignalStatus::Cleared);
        assert_eq!(
            cleared[0].kind,
            IncidentKind::from_well_known(BITCOIN_RPC_UNREACHABLE)
        );
    }

    #[test]
    fn cleared_is_not_re_emitted_when_already_cleared() {
        let mut rm = FakeReadModels::default();
        set_all_critical(&mut rm, t0());

        let rule = BitcoinRpcUnreachableRule::new();
        let subject = node();
        let i0 = Instant::now();

        // Open: condition holds long enough; we emit Active.
        let _ = rule.evaluate(ctx(&rm, &subject, t0(), i0)).unwrap();
        let _ = rule
            .evaluate(ctx(
                &rm,
                &subject,
                t0() + chrono::Duration::seconds(60),
                step(i0, 60),
            ))
            .unwrap();

        // One target recovers — Cleared once.
        rm.set_health(
            &node(),
            "bitcoin.rpc.getnetworkinfo",
            HealthStatus::Ok,
            t0() + chrono::Duration::seconds(90),
        );
        let cleared = rule
            .evaluate(ctx(
                &rm,
                &subject,
                t0() + chrono::Duration::seconds(90),
                step(i0, 90),
            ))
            .unwrap();
        assert_eq!(cleared.len(), 1);

        // Next tick with the same state — no draft (we already cleared).
        let again = rule
            .evaluate(ctx(
                &rm,
                &subject,
                t0() + chrono::Duration::seconds(120),
                step(i0, 120),
            ))
            .unwrap();
        assert!(again.is_empty());
    }

    #[test]
    fn non_bitcoin_subject_emits_nothing() {
        let rm = FakeReadModels::default();
        let rule = BitcoinRpcUnreachableRule::new();
        let lnd = EntityRef::LndNode(crate::shared::types::LndNodeId("ln1".into()));
        let drafts = rule.evaluate(ctx(&rm, &lnd, t0(), Instant::now())).unwrap();
        assert!(drafts.is_empty());
    }
}
