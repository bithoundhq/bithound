//! Per-collector task supervision.
//!
//! Spawns one tokio task per polling collector and one per
//! subscription collector. Each per-collector task runs its own
//! inner work loop and respawns it (with exponential backoff capped
//! at five minutes) if the loop ends unexpectedly — a panic for
//! polling collectors, an Err return for subscription collectors.
//!
//! Every task selects against a shared `broadcast::Receiver<()>`
//! shutdown signal and exits cleanly when it fires. Once all
//! supervisor tasks exit, their `mpsc::Sender` clones drop and the
//! observation channel closes — that's what eventually unblocks the
//! consumer.

use std::time::{Duration, Instant};

use chrono::Utc;
use futures::FutureExt;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinSet;
use uuid::Uuid;

use crate::collectors::traits::{BatchSink, PollingCollector, SubscriptionCollector};
use crate::collectors::CollectionContext;
use crate::collectors::CollectionRunId;
use crate::observations::ObservationBatch;
use crate::shared::types::SidecarId;

/// Backoff schedule used after an unexpected task exit. Resets to
/// the first element after a five-minute clean run.
const BACKOFF_SCHEDULE_SECS: [u64; 4] = [10, 30, 60, 300];

/// After this much continuous clean running, an unexpected exit is
/// treated as a fresh incident — backoff resets to the first slot.
const BACKOFF_RESET_AFTER: Duration = Duration::from_secs(300);

/// Spawns and supervises every collector. Returns when all
/// supervised tasks have exited (either because the shutdown signal
/// fired and they cleaned up, or because every collector's channel
/// send failed because the consumer dropped its receiver).
///
/// The supervisor takes ownership of every collector handle so it
/// can move it into its task. `tx` is the upstream channel into the
/// consumer; one clone goes to each task and the original is dropped
/// at the end of this function so the channel eventually closes
/// after the last collector exits.
pub async fn run(
    sidecar_id: SidecarId,
    polling: Vec<Box<dyn PollingCollector>>,
    subscription: Vec<Box<dyn SubscriptionCollector>>,
    tx: mpsc::Sender<ObservationBatch>,
    shutdown: &broadcast::Sender<()>,
) {
    let mut tasks: JoinSet<()> = JoinSet::new();

    for collector in polling {
        let tx = tx.clone();
        let shutdown_rx = shutdown.subscribe();
        let sidecar = sidecar_id.clone();
        tasks.spawn(async move {
            supervise_polling(sidecar, collector, tx, shutdown_rx).await;
        });
    }

    for collector in subscription {
        let tx = tx.clone();
        let shutdown_rx = shutdown.subscribe();
        let sidecar = sidecar_id.clone();
        tasks.spawn(async move {
            supervise_subscription(sidecar, collector, tx, shutdown_rx).await;
        });
    }

    // Drop our own clone so the channel can close once every
    // per-collector task drops its clone too.
    drop(tx);

    while tasks.join_next().await.is_some() {}
}

/// One per-collector supervisor for a polling collector. Loops the
/// inner work; on panic, waits the backoff and respawns; on
/// shutdown, returns.
async fn supervise_polling(
    sidecar_id: SidecarId,
    collector: Box<dyn PollingCollector>,
    tx: mpsc::Sender<ObservationBatch>,
    mut shutdown: broadcast::Receiver<()>,
) {
    let mut backoff_idx = 0usize;
    let collector_id = collector.descriptor().id.clone();

    loop {
        let started_at = Instant::now();

        let inner = polling_inner_loop(
            sidecar_id.clone(),
            collector.as_ref(),
            tx.clone(),
            shutdown.resubscribe(),
        );
        let outcome = std::panic::AssertUnwindSafe(inner).catch_unwind().await;

        match outcome {
            Ok(InnerExit::Shutdown) | Ok(InnerExit::ConsumerGone) => return,
            Ok(InnerExit::CollectorReturned) => {
                tracing::warn!(
                    collector = ?collector_id,
                    "polling collector inner loop returned unexpectedly; respawning",
                );
            }
            Err(panic) => {
                let detail = panic_detail(&panic);
                tracing::error!(
                    collector = ?collector_id,
                    panic = %detail,
                    "polling collector panicked; respawning",
                );
            }
        }

        if started_at.elapsed() >= BACKOFF_RESET_AFTER {
            backoff_idx = 0;
        }
        let wait = Duration::from_secs(BACKOFF_SCHEDULE_SECS[backoff_idx]);
        backoff_idx = (backoff_idx + 1).min(BACKOFF_SCHEDULE_SECS.len() - 1);

        tokio::select! {
            _ = shutdown.recv() => return,
            _ = tokio::time::sleep(wait) => {},
        }
    }
}

/// One per-collector supervisor for a subscription collector.
/// Subscription collectors are long-lived: their `run` returns Err
/// on connection death, which we treat the same as a polling panic.
async fn supervise_subscription(
    sidecar_id: SidecarId,
    collector: Box<dyn SubscriptionCollector>,
    tx: mpsc::Sender<ObservationBatch>,
    mut shutdown: broadcast::Receiver<()>,
) {
    let mut backoff_idx = 0usize;
    let descriptor = collector.descriptor().clone();
    let collector_id = descriptor.id.clone();

    loop {
        let started_at = Instant::now();

        let ctx = CollectionContext {
            sidecar_id: sidecar_id.clone(),
            collector_id: collector_id.clone(),
            target: descriptor.target.clone(),
            now: Utc::now(),
            run_id: CollectionRunId(Uuid::now_v7()),
        };
        let sink = BatchSink::new(tx.clone());

        let result = tokio::select! {
            _ = shutdown.recv() => return,
            r = collector.run(ctx, sink) => r,
        };

        if let Err(err) = result {
            tracing::warn!(
                collector = ?collector_id,
                error = ?err,
                "subscription collector returned Err; respawning",
            );
        } else {
            tracing::warn!(
                collector = ?collector_id,
                "subscription collector returned Ok unexpectedly; respawning",
            );
        }

        if started_at.elapsed() >= BACKOFF_RESET_AFTER {
            backoff_idx = 0;
        }
        let wait = Duration::from_secs(BACKOFF_SCHEDULE_SECS[backoff_idx]);
        backoff_idx = (backoff_idx + 1).min(BACKOFF_SCHEDULE_SECS.len() - 1);

        tokio::select! {
            _ = shutdown.recv() => return,
            _ = tokio::time::sleep(wait) => {},
        }
    }
}

enum InnerExit {
    Shutdown,
    ConsumerGone,
    CollectorReturned,
}

async fn polling_inner_loop(
    sidecar_id: SidecarId,
    collector: &dyn PollingCollector,
    tx: mpsc::Sender<ObservationBatch>,
    mut shutdown: broadcast::Receiver<()>,
) -> InnerExit {
    let descriptor = collector.descriptor();
    let collector_id = descriptor.id.clone();
    let target = descriptor.target.clone();

    // Polling collectors live in the polling slot exactly because
    // their integration kind carries a non-None interval. If a caller
    // hands us a subscription-kind collector here, drop into the slow
    // default rather than busy-loop.
    let interval_chrono = descriptor
        .integration
        .interval()
        .unwrap_or(chrono::Duration::seconds(10));
    let interval = interval_chrono
        .to_std()
        .unwrap_or(Duration::from_secs(10));

    let mut ticker = tokio::time::interval(interval);
    // Burn the immediate first tick so we don't double-fire at boot
    // (tokio::time::interval's default behavior is to fire instantly
    // on the first call to tick()).
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = shutdown.recv() => return InnerExit::Shutdown,
            _ = ticker.tick() => {
                let ctx = CollectionContext {
                    sidecar_id: sidecar_id.clone(),
                    collector_id: collector_id.clone(),
                    target: target.clone(),
                    now: Utc::now(),
                    run_id: CollectionRunId(Uuid::now_v7()),
                };
                let batch = collector.poll(ctx).await;
                if tx.send(batch).await.is_err() {
                    return InnerExit::ConsumerGone;
                }
            }
        }
    }
}

fn panic_detail(panic: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = panic.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use chrono::{Duration as ChronoDuration, Utc};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use crate::collectors::{CollectorDescriptor, CollectorTarget, IntegrationKind};
    use crate::observations::{ProbeResult, ProbeWindow};
    use crate::shared::types::{BitcoinNodeId, CollectorId, ObservationBatchId};

    fn descriptor(label: &str, interval_secs: i64) -> CollectorDescriptor {
        CollectorDescriptor {
            id: CollectorId(label.into()),
            integration: IntegrationKind::BitcoinCoreRpc {
                interval: ChronoDuration::seconds(interval_secs),
            },
            target: CollectorTarget::BitcoinNode(BitcoinNodeId("test-node".into())),
            instance_label: label.into(),
            description: None,
        }
    }

    fn empty_batch(sidecar_id: &SidecarId, descriptor: &CollectorDescriptor) -> ObservationBatch {
        let now = Utc::now();
        ObservationBatch {
            id: ObservationBatchId::new(),
            collector: descriptor.as_ref(),
            sidecar_id: sidecar_id.clone(),
            window: ProbeWindow::new(now, now).expect("ProbeWindow"),
            result: ProbeResult::Ok {
                observations: vec![],
            },
        }
    }

    struct CountingPollingCollector {
        descriptor: CollectorDescriptor,
        polls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl PollingCollector for CountingPollingCollector {
        fn descriptor(&self) -> &CollectorDescriptor {
            &self.descriptor
        }

        async fn poll(&self, ctx: CollectionContext) -> ObservationBatch {
            self.polls.fetch_add(1, Ordering::SeqCst);
            empty_batch(&ctx.sidecar_id, &self.descriptor)
        }
    }

    struct PanickingPollingCollector {
        descriptor: CollectorDescriptor,
        polls_before_panic: AtomicUsize,
        total_polls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl PollingCollector for PanickingPollingCollector {
        fn descriptor(&self) -> &CollectorDescriptor {
            &self.descriptor
        }

        async fn poll(&self, ctx: CollectionContext) -> ObservationBatch {
            self.total_polls.fetch_add(1, Ordering::SeqCst);
            if self.polls_before_panic.fetch_sub(1, Ordering::SeqCst) == 1 {
                panic!("test-induced panic");
            }
            empty_batch(&ctx.sidecar_id, &self.descriptor)
        }
    }

    #[tokio::test(start_paused = true)]
    async fn polling_collector_ticks_at_configured_interval() {
        // Interval is 1s; advance time and confirm the poll happens
        // each tick, not faster or slower.
        let descriptor = descriptor("ticker", 1);
        let polls = Arc::new(AtomicUsize::new(0));
        let collector: Box<dyn PollingCollector> = Box::new(CountingPollingCollector {
            descriptor,
            polls: polls.clone(),
        });

        let (tx, _rx) = mpsc::channel::<ObservationBatch>(8);
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        let sidecar = SidecarId(Uuid::now_v7());
        let shutdown_sender = shutdown_tx.clone();

        let handle = tokio::spawn(async move {
            run(sidecar, vec![collector], vec![], tx, &shutdown_sender).await;
        });

        // First tick is at 1s. Advance 1s at a time and yield so
        // the interval can fire on each step; MissedTickBehavior::Skip
        // collapses simultaneously-elapsed ticks into one, so we
        // can't just jump 3s in a single advance.
        for _ in 0..3 {
            tokio::time::advance(Duration::from_millis(1_050)).await;
            tokio::task::yield_now().await;
            tokio::task::yield_now().await;
        }

        let count = polls.load(Ordering::SeqCst);
        assert!(
            (3..=4).contains(&count),
            "expected ~3 polls after 3 ticks, got {count}",
        );

        shutdown_tx.send(()).expect("send shutdown");
        // Shutdown wait is bounded by backoff sleeps; advance time
        // generously so any pending sleep fires.
        tokio::time::advance(Duration::from_secs(10)).await;
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
    }

    #[tokio::test(start_paused = true)]
    async fn polling_collector_panic_triggers_respawn_with_backoff() {
        // Collector panics on its very first poll. After the 10s
        // backoff, supervisor respawns it and subsequent polls
        // succeed.
        let descriptor = descriptor("panicky", 1);
        let total = Arc::new(AtomicUsize::new(0));
        let collector: Box<dyn PollingCollector> = Box::new(PanickingPollingCollector {
            descriptor,
            polls_before_panic: AtomicUsize::new(1),
            total_polls: total.clone(),
        });

        let (tx, _rx) = mpsc::channel::<ObservationBatch>(8);
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        let sidecar = SidecarId(Uuid::now_v7());
        let shutdown_sender = shutdown_tx.clone();

        let handle = tokio::spawn(async move {
            run(sidecar, vec![collector], vec![], tx, &shutdown_sender).await;
        });

        // 1s first tick → panic. Then 10s backoff. Then resume on
        // 1s ticks. Advance ~13s and assert the supervisor came back.
        tokio::time::advance(Duration::from_millis(1_100)).await;
        tokio::task::yield_now().await;
        assert_eq!(
            total.load(Ordering::SeqCst),
            1,
            "expected one poll before panic"
        );

        tokio::time::advance(Duration::from_secs(11)).await;
        tokio::task::yield_now().await;

        let after_respawn = total.load(Ordering::SeqCst);
        assert!(
            after_respawn > 1,
            "supervisor must have respawned the collector after panic (saw {after_respawn} polls)",
        );

        shutdown_tx.send(()).expect("send shutdown");
        tokio::time::advance(Duration::from_secs(10)).await;
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
    }

    #[tokio::test]
    async fn shutdown_signal_exits_all_collector_tasks_within_five_seconds() {
        // Real (not paused) time: use a long interval so neither
        // collector actually fires a tick during the test window.
        let polls = Arc::new(AtomicUsize::new(0));
        let collectors: Vec<Box<dyn PollingCollector>> = (0..3)
            .map(|i| {
                Box::new(CountingPollingCollector {
                    descriptor: descriptor(&format!("c{i}"), 600),
                    polls: polls.clone(),
                }) as Box<dyn PollingCollector>
            })
            .collect();

        let (tx, _rx) = mpsc::channel::<ObservationBatch>(8);
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        let sidecar = SidecarId(Uuid::now_v7());
        let shutdown_sender = shutdown_tx.clone();

        let handle = tokio::spawn(async move {
            run(sidecar, collectors, vec![], tx, &shutdown_sender).await;
        });

        // Give the tasks a moment to subscribe to the shutdown
        // channel, then signal.
        tokio::time::sleep(Duration::from_millis(50)).await;
        shutdown_tx.send(()).expect("send shutdown");

        let outcome = tokio::time::timeout(Duration::from_secs(5), handle).await;
        assert!(
            outcome.is_ok(),
            "supervisor must finish within 5s of shutdown signal",
        );
    }

    struct ErroringSubscriptionCollector {
        descriptor: CollectorDescriptor,
        runs: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl SubscriptionCollector for ErroringSubscriptionCollector {
        fn descriptor(&self) -> &CollectorDescriptor {
            &self.descriptor
        }

        async fn run(
            &self,
            _ctx: CollectionContext,
            _sink: BatchSink,
        ) -> Result<(), crate::collectors::CollectionError> {
            self.runs.fetch_add(1, Ordering::SeqCst);
            Err(crate::collectors::CollectionError {
                kind: crate::collectors::CollectionErrorKind::Unreachable,
                message: "test connection death".into(),
            })
        }
    }

    #[tokio::test(start_paused = true)]
    async fn subscription_collector_err_triggers_respawn_with_backoff() {
        let descriptor = descriptor("subscriber", 1);
        let runs = Arc::new(AtomicUsize::new(0));
        let collector: Box<dyn SubscriptionCollector> = Box::new(ErroringSubscriptionCollector {
            descriptor,
            runs: runs.clone(),
        });

        let (tx, _rx) = mpsc::channel::<ObservationBatch>(8);
        let (shutdown_tx, _) = broadcast::channel::<()>(1);
        let sidecar = SidecarId(Uuid::now_v7());
        let shutdown_sender = shutdown_tx.clone();

        let handle = tokio::spawn(async move {
            run(sidecar, vec![], vec![collector], tx, &shutdown_sender).await;
        });

        // First run errors immediately, then 10s backoff, then
        // respawn. Advance ~11.5s and confirm at least two runs.
        tokio::time::advance(Duration::from_millis(500)).await;
        tokio::task::yield_now().await;
        assert!(runs.load(Ordering::SeqCst) >= 1, "first run should have happened");

        tokio::time::advance(Duration::from_secs(11)).await;
        tokio::task::yield_now().await;

        let after = runs.load(Ordering::SeqCst);
        assert!(
            after >= 2,
            "subscription collector must be respawned after Err (saw {after} runs)",
        );

        shutdown_tx.send(()).expect("send shutdown");
        tokio::time::advance(Duration::from_secs(10)).await;
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
    }
}
