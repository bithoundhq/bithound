//! `bitcoin.no_peers` — fires when a Bitcoin node has zero outbound
//! peers for a sustained interval and the operator hasn't disabled
//! networking. A node with no outbound peers can't follow the chain.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::diagnostics::traits::DiagnosticRule;
use crate::diagnostics::types::{DiagnosticContext, IncidentSignalDraft};
use crate::incidents::well_known::BITCOIN_NO_PEERS;
use crate::incidents::IncidentKind;
use crate::observations::{Confidence, SignalName, SignalSeverity, SignalStatus};
use crate::read_models::StateReadModelExt;
use crate::shared::types::{BitcoinNodeId, EntityRef};

/// How long the zero-outbound condition must hold before we open
/// an incident.
const DEFAULT_DEBOUNCE: Duration = Duration::from_secs(60);

/// Idle subjects (no firing condition, no open incident) are evicted from
/// the in-memory map after this long; keeps the rule's footprint bounded
/// in long-lived sidecars whose subject set churns.
const STATE_RETENTION: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Default)]
struct SubjectState {
    /// When the firing condition first held continuously for this
    /// subject, on the monotonic clock. `None` while the condition is
    /// not held.
    first_seen_at: Option<Instant>,
    /// Whether we've already emitted an `Active` draft for the
    /// current outage.
    active_emitted: bool,
    /// Last evaluation `monotonic_now` for this subject. Drives
    /// pruning of stale idle entries.
    last_touched_at: Option<Instant>,
}

/// `BitcoinNoPeersRule` emits the `bitcoin.no_peers` kind when a
/// Bitcoin node reports `connections_out == 0` AND
/// `networkactive == true` (i.e. the operator has not deliberately
/// disabled networking) for at least [`DEFAULT_DEBOUNCE`].
#[derive(Debug)]
pub struct BitcoinNoPeersRule {
    debounce: Duration,
    state: Mutex<HashMap<EntityRef, SubjectState>>,
}

impl Default for BitcoinNoPeersRule {
    fn default() -> Self {
        Self::with_debounce(DEFAULT_DEBOUNCE)
    }
}

impl BitcoinNoPeersRule {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_debounce(debounce: Duration) -> Self {
        Self {
            debounce,
            state: Mutex::new(HashMap::new()),
        }
    }
}

/// Three-way classification of the firing condition for this subject.
#[derive(Debug, PartialEq, Eq)]
enum Condition {
    /// All inputs read; condition holds (zero outbound + networkactive).
    Firing,
    /// All inputs read; condition does not hold.
    NotFiring,
    /// At least one input is missing or unknown. We treat this as
    /// "no information"; do not advance the rule's state machine.
    Unknown,
}

fn classify(ctx: &DiagnosticContext<'_>, node: &BitcoinNodeId) -> Condition {
    let Some(network) = ctx.state.bitcoin_network(node) else {
        return Condition::Unknown;
    };
    let Some(peers) = ctx.state.bitcoin_peer_summary(node) else {
        return Condition::Unknown;
    };

    let Some(network_active) = network.value.network_active else {
        // Networking enable/disable not reported — can't tell whether
        // a zero outbound count is intentional. Treat as unknown.
        return Condition::Unknown;
    };
    if !network_active {
        return Condition::NotFiring;
    }

    // Prefer the network state's connections_out; fall back to the
    // peer summary's outbound_count if the collector populated only
    // one of them.
    let outbound = network.value.connections_out.or(peers.value.outbound_count);
    let Some(outbound) = outbound else {
        return Condition::Unknown;
    };

    if outbound == 0 {
        Condition::Firing
    } else {
        Condition::NotFiring
    }
}

impl DiagnosticRule for BitcoinNoPeersRule {
    fn id(&self) -> &'static str {
        "bitcoin.no_peers"
    }

    fn evaluate(&self, ctx: DiagnosticContext<'_>) -> Result<Vec<IncidentSignalDraft>> {
        let node_id = match ctx.subject {
            EntityRef::BitcoinNode(id) => id.clone(),
            _ => return Ok(vec![]),
        };

        let condition = classify(&ctx, &node_id);

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

        match condition {
            Condition::Firing => {
                let first = *entry.first_seen_at.get_or_insert(ctx.monotonic_now);
                let held = ctx.monotonic_now.saturating_duration_since(first);

                if !entry.active_emitted && held >= self.debounce {
                    entry.active_emitted = true;
                    drafts.push(build_draft(ctx.subject, SignalStatus::Active));
                }
            }
            Condition::NotFiring => {
                entry.first_seen_at = None;
                if entry.active_emitted {
                    entry.active_emitted = false;
                    drafts.push(build_draft(ctx.subject, SignalStatus::Cleared));
                }
            }
            Condition::Unknown => {
                // No new information — leave state untouched. A long
                // missing-state window doesn't open an incident on its
                // own; that's the heartbeat rule's job in V0.1.
            }
        }

        Ok(drafts)
    }
}

/// Recover the rule's state lock through poisoning so one panicking
/// evaluation doesn't cascade into a permanently broken rule. The
/// consumer task's `catch_unwind` wrapper logs the original panic;
/// future ticks rebuild the firing/clearing counters from observed
/// state.
fn lock_state(
    mutex: &Mutex<HashMap<EntityRef, SubjectState>>,
) -> std::sync::MutexGuard<'_, HashMap<EntityRef, SubjectState>> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
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
    let kind = IncidentKind::from_well_known(BITCOIN_NO_PEERS);
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
    use crate::observations::{BitcoinNetworkState, BitcoinPeerSummaryState, StateObservation};
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

    fn network_state(connections_out: u64, network_active: bool) -> StateObservation {
        StateObservation::BitcoinNetwork(BitcoinNetworkState {
            version: 0,
            subversion: String::new(),
            protocol_version: 0,
            connections: connections_out,
            connections_in: Some(0),
            connections_out: Some(connections_out),
            network_active: Some(network_active),
        })
    }

    fn peer_summary(outbound: u64) -> StateObservation {
        StateObservation::BitcoinPeerSummary(BitcoinPeerSummaryState {
            peer_count: outbound,
            inbound_count: Some(0),
            outbound_count: Some(outbound),
            block_relay_only_count: Some(0),
        })
    }

    fn set_zero_peers_active(rm: &mut FakeReadModels, observed_at: DateTime<Utc>) {
        rm.set_state(&node(), network_state(0, true), observed_at);
        rm.set_state(&node(), peer_summary(0), observed_at);
    }

    #[test]
    fn id_is_kind_name() {
        let rule = BitcoinNoPeersRule::new();
        assert_eq!(rule.id(), "bitcoin.no_peers");
    }

    #[test]
    fn zero_outbound_for_sixty_seconds_emits_active() {
        let mut rm = FakeReadModels::default();
        set_zero_peers_active(&mut rm, t0());

        let rule = BitcoinNoPeersRule::new();
        let subject = node();
        let i0 = Instant::now();

        // Within debounce → no draft.
        assert!(rule
            .evaluate(ctx(&rm, &subject, t0(), i0))
            .unwrap()
            .is_empty());

        let drafts = rule
            .evaluate(ctx(
                &rm,
                &subject,
                t0() + chrono::Duration::seconds(60),
                step(i0, 60),
            ))
            .unwrap();
        assert_eq!(drafts.len(), 1);
        let d = &drafts[0];
        assert_eq!(d.status, SignalStatus::Active);
        assert_eq!(d.kind, IncidentKind::from_well_known(BITCOIN_NO_PEERS));
        assert_eq!(d.severity, SignalSeverity::Critical);
        assert_eq!(d.confidence, Confidence::High);
    }

    #[test]
    fn outbound_recovers_after_active_emits_cleared() {
        let mut rm = FakeReadModels::default();
        set_zero_peers_active(&mut rm, t0());

        let rule = BitcoinNoPeersRule::new();
        let subject = node();
        let i0 = Instant::now();

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

        // One outbound peer reappears.
        rm.set_state(
            &node(),
            network_state(1, true),
            t0() + chrono::Duration::seconds(90),
        );
        rm.set_state(
            &node(),
            peer_summary(1),
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
    }

    #[test]
    fn networkactive_false_emits_nothing_even_at_zero() {
        let mut rm = FakeReadModels::default();
        rm.set_state(&node(), network_state(0, false), t0());
        rm.set_state(&node(), peer_summary(0), t0());

        let rule = BitcoinNoPeersRule::new();
        let subject = node();
        let i0 = Instant::now();

        for offset in [0u64, 60, 600] {
            let drafts = rule
                .evaluate(ctx(
                    &rm,
                    &subject,
                    t0() + chrono::Duration::seconds(offset as i64),
                    step(i0, offset),
                ))
                .unwrap();
            assert!(
                drafts.is_empty(),
                "operator disabled networking → no incident",
            );
        }
    }

    #[test]
    fn brief_outage_under_sixty_seconds_emits_nothing() {
        let mut rm = FakeReadModels::default();
        set_zero_peers_active(&mut rm, t0());

        let rule = BitcoinNoPeersRule::new();
        let subject = node();
        let i0 = Instant::now();

        // 30s later peers recover before the 60s debounce.
        assert!(rule
            .evaluate(ctx(&rm, &subject, t0(), i0))
            .unwrap()
            .is_empty());
        rm.set_state(
            &node(),
            network_state(3, true),
            t0() + chrono::Duration::seconds(30),
        );
        rm.set_state(
            &node(),
            peer_summary(3),
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
        assert!(drafts.is_empty());
    }

    #[test]
    fn unknown_state_does_not_advance_timer() {
        // No bitcoin_network state set on the read-model — classify
        // returns Unknown. The rule should stay quiet across multiple
        // ticks even after the debounce window has elapsed.
        let rm = FakeReadModels::default();
        let rule = BitcoinNoPeersRule::new();
        let subject = node();
        let i0 = Instant::now();
        for offset in [0u64, 60, 600] {
            assert!(rule
                .evaluate(ctx(
                    &rm,
                    &subject,
                    t0() + chrono::Duration::seconds(offset as i64),
                    step(i0, offset),
                ))
                .unwrap()
                .is_empty());
        }
    }

    #[test]
    fn non_bitcoin_subject_emits_nothing() {
        let rm = FakeReadModels::default();
        let rule = BitcoinNoPeersRule::new();
        let lnd = EntityRef::LndNode(crate::shared::types::LndNodeId("ln1".into()));
        let drafts = rule.evaluate(ctx(&rm, &lnd, t0(), Instant::now())).unwrap();
        assert!(drafts.is_empty());
    }
}
