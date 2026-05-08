//! Generic primitives for periodic telemetry collection.
//!
//! A [`Probe`] is a unit that asynchronously collects one value. [`spawn_probe`]
//! drives it on a fixed cadence and publishes the latest [`ProbeSnapshot`] to a
//! `tokio::sync::watch` channel — single writer, many readers, overwrite-on-send
//! semantics. Domain modules (e.g. `bitcoin::telemetry`) compose several probes
//! into an aggregate snapshot.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use tokio::{sync::watch, task::JoinSet, time::MissedTickBehavior};

/// Failure modes reported by a [`Probe::collect`] call.
///
/// `Timeout` is produced by [`spawn_probe`] when a collection exceeds its
/// configured deadline — probes do not raise it themselves.
#[derive(Debug, thiserror::Error)]
pub enum ProbeError {
    #[error("Transport error: {0}")]
    Transport(String),
    #[error("Decode: {0}")]
    Decode(String),
    #[error("Timeout")]
    Timeout,
}

/// A unit that asynchronously collects one telemetry value.
///
/// Implementations should be cheap to construct and hold any backend handles
/// (RPC clients, OS handles) behind an `Arc` so the probe can be moved into a
/// spawned task. Errors are surfaced as [`ProbeError`]; panics inside `collect`
/// are caught by the owning [`JoinSet`] but terminate the probe's loop.
#[async_trait]
pub trait Probe: Send + Sync + 'static {
    /// Value type produced on a successful collection.
    type Output: Clone + Send + Sync + 'static;

    /// Collect a single observation.
    async fn collect(&self) -> Result<Self::Output, ProbeError>;
}

/// Per-probe runtime configuration.
///
/// `interval` is the cadence of the collection loop. `timeout` bounds a single
/// `collect` call before [`spawn_probe`] cancels it and emits `Failed`. `ttl`
/// is *not* enforced inside the loop; it is applied at projection time by
/// [`evaluate_ttl`] to demote a recent `Success` to `Stale` once it has aged.
#[derive(Debug, Clone)]
pub struct ProbeConfig {
    pub interval: tokio::time::Duration,
    pub timeout: tokio::time::Duration,
    pub ttl: Duration,
}

/// Latest known state of a single probe.
///
/// - `Missing` — initial value, before any observation has been published.
/// - `Success` — most recent successful collection, freshness not yet evaluated.
/// - `Stale`   — a `Success` whose `observed_at` predates `now() - ttl`;
///   produced by [`evaluate_ttl`] at projection time, never written by the
///   probe loop directly.
/// - `Failed`  — the most recent collection raised an error or timed out.
#[derive(Debug, Clone)]
pub enum ProbeSnapshot<T> {
    Success {
        value: T,
        observed_at: DateTime<Utc>,
    },
    Stale {
        value: T,
        observed_at: DateTime<Utc>,
    },
    Failed {
        last_error: String,
        failed_at: DateTime<Utc>,
    },
    Missing,
}

/// Spawn a probe loop on `tasks` and return a receiver of its latest snapshot.
///
/// The spawned task ticks at `config.interval`, calls [`Probe::collect`] with a
/// `config.timeout` budget, and publishes the resulting [`ProbeSnapshot`] to a
/// `watch` channel. A snapshot is sent on every tick — even when the value is
/// unchanged — but `watch` overwrites in place, so consumers only ever observe
/// the most recent value.
///
/// Missed ticks use `MissedTickBehavior::Delay` to avoid burst-firing after a
/// slow `collect`. The task exits cleanly when every receiver has been dropped
/// or when `tasks` is shut down.
pub fn spawn_probe<P>(
    probe: P,
    config: ProbeConfig,
    tasks: &mut JoinSet<()>,
) -> watch::Receiver<ProbeSnapshot<P::Output>>
where
    P: Probe,
{
    let (tx, rx) = watch::channel(ProbeSnapshot::Missing);

    tasks.spawn(async move {
        let mut interval = tokio::time::interval(config.interval);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            interval.tick().await;

            let observed_at = Utc::now();
            let snap = match tokio::time::timeout(config.timeout, probe.collect()).await {
                Ok(Ok(value)) => ProbeSnapshot::Success { value, observed_at },
                Ok(Err(e)) => ProbeSnapshot::Failed {
                    last_error: e.to_string(),
                    failed_at: observed_at,
                },
                Err(_) => ProbeSnapshot::Failed {
                    last_error: ProbeError::Timeout.to_string(),
                    failed_at: observed_at,
                },
            };

            if tx.send(snap).is_err() {
                break;
            }
        }
    });

    rx
}

/// Demote a `Success` snapshot to `Stale` once its `observed_at` is older than
/// `ttl` relative to `now`.
///
/// All other variants are returned unchanged. Apply this at projection time —
/// i.e. when assembling a domain snapshot for consumers — rather than inside
/// the probe loop, so staleness is recomputed against the *read-time* clock
/// rather than frozen at write time.
pub fn evaluate_ttl<T>(
    snap: ProbeSnapshot<T>,
    ttl: Duration,
    now: DateTime<Utc>,
) -> ProbeSnapshot<T> {
    match snap {
        ProbeSnapshot::Success { value, observed_at } if now - observed_at > ttl => {
            ProbeSnapshot::Stale { value, observed_at }
        }
        other => other,
    }
}
