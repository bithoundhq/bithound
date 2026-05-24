//! `lnd.chain_backend_lag` — fires when LND's view of the chain tip
//! falls behind bitcoind's view by more than `lag_blocks_threshold`
//! for `lag_persist_seconds`. Cross-source correlation: reads two
//! existing state observations (LndNodeState.block_height from the
//! polling collector, BitcoinBlockchainState.blocks from the V0
//! Bitcoin RPC collector) and emits drafts when the heights diverge.
//!
//! The rule treats the configured `bitcoind_id` as the correlation
//! target. If the runtime hasn't picked one (e.g. multiple bitcoinds
//! configured but no correlation target chosen), the rule skips
//! firing for the LND node — better to stay silent than draft against
//! the wrong correlation. A future ticket can promote this to an
//! `Active` draft with `Misconfigured` severity if operators want a
//! louder signal for unconfigured correlations.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::diagnostics::traits::DiagnosticRule;
use crate::diagnostics::types::{DiagnosticContext, IncidentSignalDraft};
use crate::incidents::well_known::LND_CHAIN_BACKEND_LAG;
use crate::incidents::IncidentKind;
use crate::observations::state::well_known::{BITCOIN_BLOCKCHAIN, LND_NODE};
use crate::observations::{
    Confidence, SignalName, SignalSeverity, SignalStatus, StateName, StateObservation,
};
use crate::shared::types::{BitcoinNodeId, EntityRef};

const DEFAULT_LAG_BLOCKS_THRESHOLD: u64 = 2;
const DEFAULT_LAG_PERSIST: Duration = Duration::from_secs(60);
const STATE_RETENTION: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Default)]
struct SubjectState {
    first_lagged_at: Option<Instant>,
    active_emitted: bool,
    last_touched_at: Option<Instant>,
}

#[derive(Debug)]
pub struct LndChainBackendLagRule {
    correlation_target: BitcoinNodeId,
    lag_blocks_threshold: u64,
    lag_persist: Duration,
    state: Mutex<HashMap<EntityRef, SubjectState>>,
}

impl LndChainBackendLagRule {
    /// Construct with the operator-chosen bitcoind correlation target.
    /// V0.8 expects the runtime to pass the only configured bitcoind
    /// when a single bitcoind is present; multi-bitcoind deployments
    /// pass the explicit
    /// `[collectors.lnd.nodes.<id>].chain_backend_target_bitcoind_id`
    /// from the operator's config.
    pub fn new(correlation_target: BitcoinNodeId) -> Self {
        Self::with_thresholds(
            correlation_target,
            DEFAULT_LAG_BLOCKS_THRESHOLD,
            DEFAULT_LAG_PERSIST,
        )
    }

    pub fn with_thresholds(
        correlation_target: BitcoinNodeId,
        lag_blocks_threshold: u64,
        lag_persist: Duration,
    ) -> Self {
        Self {
            correlation_target,
            lag_blocks_threshold,
            lag_persist,
            state: Mutex::new(HashMap::new()),
        }
    }
}

impl DiagnosticRule for LndChainBackendLagRule {
    fn id(&self) -> &'static str {
        "lnd.chain_backend_lag"
    }

    fn evaluate(&self, ctx: DiagnosticContext<'_>) -> Result<Vec<IncidentSignalDraft>> {
        // Only meaningful for LndNode subjects.
        if !matches!(ctx.subject, EntityRef::LndNode(_)) {
            return Ok(vec![]);
        }

        // Pull LND's view of the chain tip.
        let lnd_height = match ctx
            .state
            .latest_state(ctx.subject, &StateName::from_well_known(LND_NODE))
        {
            Some(snapshot) => match snapshot.value {
                StateObservation::LndNode(s) => s.block_height,
                _ => return Ok(vec![]),
            },
            None => return Ok(vec![]),
        };

        // Pull bitcoind's view of the chain tip from the correlation
        // target.
        let bitcoind_subject = EntityRef::BitcoinNode(self.correlation_target.clone());
        let bitcoind_height = match ctx.state.latest_state(
            &bitcoind_subject,
            &StateName::from_well_known(BITCOIN_BLOCKCHAIN),
        ) {
            Some(snapshot) => match snapshot.value {
                StateObservation::BitcoinBlockchain(s) => s.blocks,
                _ => return Ok(vec![]),
            },
            None => return Ok(vec![]),
        };

        // bitcoind ahead of LND by > threshold = lag condition.
        let lagged = bitcoind_height
            .saturating_sub(lnd_height)
            > self.lag_blocks_threshold;

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

        if lagged {
            let first = *entry.first_lagged_at.get_or_insert(ctx.monotonic_now);
            let held = ctx.monotonic_now.saturating_duration_since(first);

            if !entry.active_emitted && held >= self.lag_persist {
                entry.active_emitted = true;
                drafts.push(build_draft(ctx.subject, SignalStatus::Active));
            }
        } else {
            entry.first_lagged_at = None;
            if entry.active_emitted {
                entry.active_emitted = false;
                drafts.push(build_draft(ctx.subject, SignalStatus::Cleared));
            }
        }

        Ok(drafts)
    }
}

fn build_draft(subject: &EntityRef, status: SignalStatus) -> IncidentSignalDraft {
    let kind = IncidentKind::from_well_known(LND_CHAIN_BACKEND_LAG);
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
        if entry.active_emitted {
            return true;
        }
        match entry.last_touched_at {
            Some(ts) => now.saturating_duration_since(ts) < STATE_RETENTION,
            None => true,
        }
    });
}
