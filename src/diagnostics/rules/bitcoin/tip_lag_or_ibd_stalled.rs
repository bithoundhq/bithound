//! `bitcoin.tip_lag_or_ibd_stalled` — combines incident catalog entries
//! A1 (tip lag) and A2 (IBD stall) into a single V0 rule. Both patterns
//! indicate "the node is not actually following the chain right now,"
//! and from the operator's perspective the response is the same: check
//! the node.
//!
//! Hysteresis is rule-owned: either pattern must hold across two
//! consecutive evaluations before we open an incident, and the absence
//! of both patterns must hold across two consecutive evaluations before
//! we resolve it. This prevents a single-tick flap from flooding the
//! operator with open/resolve pairs.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::diagnostics::traits::DiagnosticRule;
use crate::diagnostics::types::{DiagnosticContext, IncidentSignalDraft};
use crate::incidents::well_known::BITCOIN_TIP_LAG_OR_IBD_STALLED;
use crate::incidents::IncidentKind;
use crate::observations::{
    BitcoinBlockchainState, BitcoinPeerSummaryState, Confidence, SignalName, SignalSeverity,
    SignalStatus,
};
use crate::read_models::StateReadModelExt;
use crate::shared::types::EntityRef;

/// A1 thresholds (catalog entry "tip lag").
const A1_MAX_HEADER_BLOCK_GAP: u64 = 1000;
const A1_MIN_VERIFICATION_PROGRESS: f64 = 0.999;
const A1_MIN_PEER_COUNT: u64 = 8;

/// A2 thresholds (catalog entry "IBD stall"). The minimum-gap threshold
/// is intentionally equal to [`A1_MAX_HEADER_BLOCK_GAP`] so the
/// `[A1_MAX..A2_MIN]` band contains no dead zone: a gap of exactly 1000
/// is covered by A2 (subject to its flat-window guard) where A1's strict
/// `< 1000` no longer applies.
const A2_MIN_HEADER_BLOCK_GAP: u64 = A1_MAX_HEADER_BLOCK_GAP;
const DEFAULT_A2_FLAT_WINDOW: Duration = Duration::from_secs(5 * 60);

/// Minimum change in `verification_progress` that counts as "progress".
/// Smaller changes are treated as noise (e.g. floating-point rounding
/// across two near-identical RPC responses).
const PROGRESS_EPSILON: f64 = 1e-9;

/// Idle subjects (no firing condition, no open incident) are evicted
/// from the in-memory state map after this long without a tick, so a
/// long-lived sidecar that sees subjects come and go doesn't accumulate
/// dead entries.
const STATE_RETENTION: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Default)]
struct SubjectState {
    /// Consecutive evaluations where either A1 or A2 held.
    consecutive_firing: u32,
    /// Consecutive evaluations where neither A1 nor A2 held.
    consecutive_clearing: u32,
    /// Whether we currently have an Active draft outstanding.
    active_emitted: bool,
    /// Tracking for A2's "verification_progress flat over a window".
    /// `None` until we see a sample.
    last_progress: Option<f64>,
    /// Monotonic instant of the last meaningful change in
    /// `verification_progress` — used to test the flat-window threshold
    /// for A2.
    last_progress_change_at: Option<Instant>,
    /// Last evaluation `monotonic_now` for this subject. Drives pruning
    /// of stale idle entries.
    last_touched_at: Option<Instant>,
}

/// `BitcoinTipLagOrIbdStalledRule` is stateful (per-subject) so it can
/// debounce both the open and resolve transitions over two consecutive
/// ticks, and so it can detect A2's "verification_progress flat for a
/// multi-minute window" pattern from a stream of single-point reads.
#[derive(Debug)]
pub struct BitcoinTipLagOrIbdStalledRule {
    a2_flat_window: Duration,
    debounce_ticks: u32,
    state: HashMap<EntityRef, SubjectState>,
}

impl Default for BitcoinTipLagOrIbdStalledRule {
    fn default() -> Self {
        Self::with_settings(DEFAULT_A2_FLAT_WINDOW, 2)
    }
}

impl BitcoinTipLagOrIbdStalledRule {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_settings(a2_flat_window: Duration, debounce_ticks: u32) -> Self {
        debug_assert!(
            debounce_ticks >= 1,
            "debounce_ticks must be >= 1; 0 collapses the rule to fire-on-first-tick",
        );
        Self {
            a2_flat_window,
            debounce_ticks,
            state: HashMap::new(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Classification {
    Firing,
    NotFiring,
}

fn a1_holds(blockchain: &BitcoinBlockchainState, peers: &BitcoinPeerSummaryState) -> bool {
    if !blockchain.initial_block_download {
        return false;
    }
    let gap = blockchain.headers.saturating_sub(blockchain.blocks);
    gap < A1_MAX_HEADER_BLOCK_GAP
        && blockchain.verification_progress > A1_MIN_VERIFICATION_PROGRESS
        && peers.peer_count >= A1_MIN_PEER_COUNT
}

impl DiagnosticRule for BitcoinTipLagOrIbdStalledRule {
    fn id(&self) -> &'static str {
        "bitcoin.tip_lag_or_ibd_stalled"
    }

    fn evaluate(&mut self, ctx: DiagnosticContext<'_>) -> Result<Vec<IncidentSignalDraft>> {
        let node_id = match ctx.subject {
            EntityRef::BitcoinNode(id) => id.clone(),
            _ => return Ok(vec![]),
        };

        let Some(blockchain) = ctx.state.bitcoin_blockchain(&node_id) else {
            return Ok(vec![]);
        };
        let Some(peers) = ctx.state.bitcoin_peer_summary(&node_id) else {
            return Ok(vec![]);
        };

        prune_stale(&mut self.state, ctx.monotonic_now);
        if !self.state.contains_key(ctx.subject) {
            self.state
                .insert(ctx.subject.clone(), SubjectState::default());
        }
        let entry = self
            .state
            .get_mut(ctx.subject)
            .expect("inserted above if missing");
        entry.last_touched_at = Some(ctx.monotonic_now);
        let mut drafts: Vec<IncidentSignalDraft> = Vec::new();

        let classification = update_state_and_classify(
            entry,
            &blockchain.value,
            &peers.value,
            ctx.monotonic_now,
            self.a2_flat_window,
        );

        match classification {
            Classification::Firing => {
                entry.consecutive_clearing = 0;
                entry.consecutive_firing = entry.consecutive_firing.saturating_add(1);
                if !entry.active_emitted && entry.consecutive_firing >= self.debounce_ticks {
                    entry.active_emitted = true;
                    drafts.push(build_draft(ctx.subject, SignalStatus::Active));
                }
            }
            Classification::NotFiring => {
                entry.consecutive_firing = 0;
                entry.consecutive_clearing = entry.consecutive_clearing.saturating_add(1);
                if entry.active_emitted && entry.consecutive_clearing >= self.debounce_ticks {
                    entry.active_emitted = false;
                    drafts.push(build_draft(ctx.subject, SignalStatus::Cleared));
                }
            }
        }

        Ok(drafts)
    }
}

/// Update A2's flat-window tracker in `state` from the latest blockchain
/// observation, then decide whether either A1 or A2 holds. Single
/// caller: `evaluate` holds `&mut self.state` directly across the call,
/// and the consumer task is the only writer per ADR-S1, so the progress
/// tracker can't tear across a concurrent evaluation.
fn update_state_and_classify(
    state: &mut SubjectState,
    blockchain: &BitcoinBlockchainState,
    peers: &BitcoinPeerSummaryState,
    now: Instant,
    a2_flat_window: Duration,
) -> Classification {
    let progress = blockchain.verification_progress;
    match state.last_progress {
        None => {
            state.last_progress = Some(progress);
            state.last_progress_change_at = Some(now);
        }
        Some(prev) => {
            if (progress - prev).abs() > PROGRESS_EPSILON {
                state.last_progress = Some(progress);
                state.last_progress_change_at = Some(now);
            }
        }
    }
    let last_change = state.last_progress_change_at.unwrap_or(now);

    let a1 = a1_holds(blockchain, peers);
    let a2 = {
        let gap = blockchain.headers.saturating_sub(blockchain.blocks);
        let elapsed_since_change = now.saturating_duration_since(last_change);
        gap >= A2_MIN_HEADER_BLOCK_GAP && elapsed_since_change >= a2_flat_window
    };

    if a1 || a2 {
        Classification::Firing
    } else {
        Classification::NotFiring
    }
}

/// Drop entries that haven't been touched in [`STATE_RETENTION`] and
/// have no active incident outstanding.
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
    let kind = IncidentKind::from_well_known(BITCOIN_TIP_LAG_OR_IBD_STALLED);
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
    use crate::observations::{BitcoinBlockchainState, BitcoinPeerSummaryState, StateObservation};
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

    fn step(base: Instant, secs: u64) -> Instant {
        base + Duration::from_secs(secs)
    }

    fn blockchain_state(
        blocks: u64,
        headers: u64,
        verification_progress: f64,
        ibd: bool,
    ) -> StateObservation {
        StateObservation::BitcoinBlockchain(BitcoinBlockchainState {
            chain: "main".into(),
            blocks,
            headers,
            best_block_hash: None,
            verification_progress,
            initial_block_download: ibd,
            pruned: false,
            size_on_disk_bytes: 0,
        })
    }

    fn peer_summary(peer_count: u64) -> StateObservation {
        StateObservation::BitcoinPeerSummary(BitcoinPeerSummaryState {
            peer_count,
            inbound_count: None,
            outbound_count: None,
            block_relay_only_count: None,
        })
    }

    fn set_a1_firing(rm: &mut FakeReadModels, observed_at: DateTime<Utc>) {
        // headers - blocks = 500 (< 1000), IBD true, verification 0.9995, 10 peers.
        rm.set_state(
            &node(),
            blockchain_state(900_500, 901_000, 0.9995, true),
            observed_at,
        );
        rm.set_state(&node(), peer_summary(10), observed_at);
    }

    fn set_a1_cleared(rm: &mut FakeReadModels, observed_at: DateTime<Utc>) {
        // IBD complete — A1 cannot hold once initial_block_download flips false.
        rm.set_state(
            &node(),
            blockchain_state(901_000, 901_000, 1.0, false),
            observed_at,
        );
        rm.set_state(&node(), peer_summary(10), observed_at);
    }

    #[test]
    fn id_is_kind_name() {
        assert_eq!(
            BitcoinTipLagOrIbdStalledRule::new().id(),
            "bitcoin.tip_lag_or_ibd_stalled"
        );
    }

    #[test]
    fn single_tick_a1_does_not_open() {
        let mut rm = FakeReadModels::default();
        set_a1_firing(&mut rm, t0());

        let mut rule = BitcoinTipLagOrIbdStalledRule::new();
        let subject = node();
        let i0 = Instant::now();

        let drafts = rule.evaluate(ctx(&rm, &subject, t0(), i0)).unwrap();
        assert!(drafts.is_empty(), "single tick → no draft");
    }

    #[test]
    fn two_consecutive_a1_ticks_emit_active() {
        let mut rm = FakeReadModels::default();
        set_a1_firing(&mut rm, t0());

        let mut rule = BitcoinTipLagOrIbdStalledRule::new();
        let subject = node();
        let i0 = Instant::now();

        let _ = rule.evaluate(ctx(&rm, &subject, t0(), i0)).unwrap();

        // Update state to the same condition with a slightly later
        // observed_at to simulate a second tick.
        set_a1_firing(&mut rm, t0() + chrono::Duration::seconds(30));

        let drafts = rule
            .evaluate(ctx(
                &rm,
                &subject,
                t0() + chrono::Duration::seconds(30),
                step(i0, 30),
            ))
            .unwrap();
        assert_eq!(drafts.len(), 1);
        let d = &drafts[0];
        assert_eq!(d.status, SignalStatus::Active);
        assert_eq!(
            d.kind,
            IncidentKind::from_well_known(BITCOIN_TIP_LAG_OR_IBD_STALLED)
        );
        assert_eq!(d.severity, SignalSeverity::Critical);
        assert_eq!(d.confidence, Confidence::High);
    }

    #[test]
    fn two_consecutive_clear_ticks_after_active_emit_cleared() {
        let mut rm = FakeReadModels::default();
        set_a1_firing(&mut rm, t0());

        let mut rule = BitcoinTipLagOrIbdStalledRule::new();
        let subject = node();
        let i0 = Instant::now();

        // Open: two firing ticks.
        let _ = rule.evaluate(ctx(&rm, &subject, t0(), i0)).unwrap();
        set_a1_firing(&mut rm, t0() + chrono::Duration::seconds(30));
        let opened = rule
            .evaluate(ctx(
                &rm,
                &subject,
                t0() + chrono::Duration::seconds(30),
                step(i0, 30),
            ))
            .unwrap();
        assert_eq!(opened.len(), 1);

        // Two clearing ticks.
        set_a1_cleared(&mut rm, t0() + chrono::Duration::seconds(60));
        let clearing_tick_one = rule
            .evaluate(ctx(
                &rm,
                &subject,
                t0() + chrono::Duration::seconds(60),
                step(i0, 60),
            ))
            .unwrap();
        assert!(clearing_tick_one.is_empty(), "single clear tick → no draft");

        set_a1_cleared(&mut rm, t0() + chrono::Duration::seconds(90));
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
    }

    #[test]
    fn a2_fires_when_verification_progress_is_flat_over_window() {
        let mut rule = BitcoinTipLagOrIbdStalledRule::new();
        let subject = node();
        let mut rm = FakeReadModels::default();
        let i0 = Instant::now();

        // Tick 1 at t=0: large header/block gap, verification_progress = 0.5
        // (not A1, but A2's gap requirement is met). The rule should
        // record the initial progress sample without emitting.
        rm.set_state(&node(), blockchain_state(800_000, 900_000, 0.5, true), t0());
        rm.set_state(&node(), peer_summary(10), t0());
        let drafts = rule.evaluate(ctx(&rm, &subject, t0(), i0)).unwrap();
        assert!(drafts.is_empty(), "first tick can't satisfy flat window");

        // Tick 2 at t+5min: same verification_progress as tick 1.
        // The flat window matches and the debounce of 2 firing ticks
        // is also satisfied across these two ticks.
        let later = t0() + chrono::Duration::seconds(5 * 60);
        rm.set_state(
            &node(),
            blockchain_state(800_000, 900_000, 0.5, true),
            later,
        );
        rm.set_state(&node(), peer_summary(10), later);
        let drafts = rule
            .evaluate(ctx(&rm, &subject, later, step(i0, 5 * 60)))
            .unwrap();
        // Tick 1 set last_progress_change_at = i0 with elapsed = 0,
        // so A2 was false (0 < 5min). Tick 2 has elapsed = 5min and
        // counts as the first firing tick; one more is needed to
        // satisfy the two-tick debounce.
        assert!(
            drafts.is_empty(),
            "first firing tick after window → no draft yet"
        );

        let still_later = later + chrono::Duration::seconds(60);
        rm.set_state(
            &node(),
            blockchain_state(800_000, 900_000, 0.5, true),
            still_later,
        );
        rm.set_state(&node(), peer_summary(10), still_later);
        let drafts = rule
            .evaluate(ctx(&rm, &subject, still_later, step(i0, 6 * 60)))
            .unwrap();
        assert_eq!(drafts.len(), 1, "second firing tick → Active");
        assert_eq!(drafts[0].status, SignalStatus::Active);
    }

    #[test]
    fn a2_does_not_fire_when_progress_keeps_advancing() {
        let mut rule = BitcoinTipLagOrIbdStalledRule::new();
        let subject = node();
        let mut rm = FakeReadModels::default();
        let i0 = Instant::now();

        // verification_progress advances each tick, so A2's flat-window
        // counter resets.
        let samples = [0.50, 0.60, 0.70, 0.80, 0.90, 0.95];
        for (i, p) in samples.iter().enumerate() {
            let secs = 60 * i as u64;
            let when = t0() + chrono::Duration::seconds(secs as i64);
            rm.set_state(&node(), blockchain_state(800_000, 900_000, *p, true), when);
            rm.set_state(&node(), peer_summary(10), when);
            let drafts = rule
                .evaluate(ctx(&rm, &subject, when, step(i0, secs)))
                .unwrap();
            assert!(
                drafts.is_empty(),
                "progress advancing → no draft at tick {i}"
            );
        }
    }

    #[test]
    fn unknown_state_leaves_counters_alone() {
        let rm = FakeReadModels::default();
        let mut rule = BitcoinTipLagOrIbdStalledRule::new();
        let subject = node();
        let i0 = Instant::now();
        for offset in [0u64, 30, 60, 90] {
            let drafts = rule
                .evaluate(ctx(
                    &rm,
                    &subject,
                    t0() + chrono::Duration::seconds(offset as i64),
                    step(i0, offset),
                ))
                .unwrap();
            assert!(drafts.is_empty(), "no state → no draft at offset {offset}");
        }
    }

    #[test]
    fn non_bitcoin_subject_emits_nothing() {
        let rm = FakeReadModels::default();
        let mut rule = BitcoinTipLagOrIbdStalledRule::new();
        let lnd = EntityRef::LndNode(crate::shared::types::LndNodeId("ln1".into()));
        let drafts = rule.evaluate(ctx(&rm, &lnd, t0(), Instant::now())).unwrap();
        assert!(drafts.is_empty());
    }

    #[test]
    fn a2_does_not_fire_just_below_flat_window() {
        // Boundary test: the second tick lands one second *before* the
        // 5-minute flat window completes. A2 must NOT report firing.
        let mut rule = BitcoinTipLagOrIbdStalledRule::new();
        let subject = node();
        let mut rm = FakeReadModels::default();
        let i0 = Instant::now();

        rm.set_state(&node(), blockchain_state(800_000, 900_000, 0.5, true), t0());
        rm.set_state(&node(), peer_summary(10), t0());
        let _ = rule.evaluate(ctx(&rm, &subject, t0(), i0)).unwrap();

        let just_below = t0() + chrono::Duration::seconds(5 * 60 - 1);
        rm.set_state(
            &node(),
            blockchain_state(800_000, 900_000, 0.5, true),
            just_below,
        );
        rm.set_state(&node(), peer_summary(10), just_below);
        let drafts = rule
            .evaluate(ctx(&rm, &subject, just_below, step(i0, 5 * 60 - 1)))
            .unwrap();
        assert!(
            drafts.is_empty(),
            "below the flat window → A2 must not fire",
        );
    }

    #[test]
    fn header_block_gap_band_between_a1_and_a2_thresholds_is_covered() {
        // A1 requires gap < 1000; A2 requires gap >= 1000. At exactly
        // 1000 (with IBD finished, so A1's IBD precondition fails)
        // and verification_progress flat over the window, only A2 can
        // cover this — and it must.
        let mut rule = BitcoinTipLagOrIbdStalledRule::new();
        let subject = node();
        let mut rm = FakeReadModels::default();
        let i0 = Instant::now();

        let progress = 0.99995;
        rm.set_state(
            &node(),
            blockchain_state(800_000, 801_000, progress, false),
            t0(),
        );
        rm.set_state(&node(), peer_summary(10), t0());
        let _ = rule.evaluate(ctx(&rm, &subject, t0(), i0)).unwrap();

        // Same progress at t+5min; A2 satisfies. One more tick to debounce.
        let later = t0() + chrono::Duration::seconds(5 * 60);
        rm.set_state(
            &node(),
            blockchain_state(800_000, 801_000, progress, false),
            later,
        );
        rm.set_state(&node(), peer_summary(10), later);
        let _ = rule
            .evaluate(ctx(&rm, &subject, later, step(i0, 5 * 60)))
            .unwrap();

        let still_later = later + chrono::Duration::seconds(60);
        rm.set_state(
            &node(),
            blockchain_state(800_000, 801_000, progress, false),
            still_later,
        );
        rm.set_state(&node(), peer_summary(10), still_later);
        let drafts = rule
            .evaluate(ctx(&rm, &subject, still_later, step(i0, 6 * 60)))
            .unwrap();
        assert_eq!(drafts.len(), 1, "gap=1000 dead-zone closure must fire A2");
        assert_eq!(drafts[0].status, SignalStatus::Active);
    }
}
