//! Collector trait surface.
//!
//! Two traits, one per collection mode. Polling collectors run a request
//! per scheduled tick and always return a batch; subscription collectors
//! are spawned once with a sink and run until the connection dies or the
//! receiver is dropped.

use async_trait::async_trait;
use tokio::sync::mpsc;

use super::types::{CollectionContext, CollectionError, CollectorDescriptor};
use crate::observations::ObservationBatch;

/// Tick-driven collector. The runtime calls [`poll`] on a schedule
/// derived from [`super::IntegrationKind::interval`]. `poll` never
/// returns `Err`: every internal failure is folded into the returned
/// `ObservationBatch` via `ProbeResult::Failed { health, partial_observations, error }`.
#[async_trait]
pub trait PollingCollector: Send + Sync {
    fn descriptor(&self) -> &CollectorDescriptor;
    async fn poll(&self, ctx: CollectionContext) -> ObservationBatch;
}

/// Long-lived collector. The runtime spawns one task per implementor and
/// hands it a [`BatchSink`]. `run` returns when the connection dies
/// unrecoverably or the sink is closed; the runtime decides whether to
/// re-spawn with backoff.
#[async_trait]
pub trait SubscriptionCollector: Send + Sync {
    fn descriptor(&self) -> &CollectorDescriptor;
    async fn run(&self, ctx: CollectionContext, sink: BatchSink) -> Result<(), CollectionError>;
}

/// Handle the runtime passes to subscription collectors. Wraps a
/// bounded `tokio::sync::mpsc::Sender`. Cloneable so collectors can
/// hand it to sub-tasks.
#[derive(Debug, Clone)]
pub struct BatchSink {
    tx: mpsc::Sender<ObservationBatch>,
}

impl BatchSink {
    pub fn new(tx: mpsc::Sender<ObservationBatch>) -> Self {
        Self { tx }
    }

    /// Send a batch downstream. Returns [`SinkError::Closed`] if the
    /// receiver has been dropped — the runtime has stopped consuming
    /// from this sink and the collector should shut down.
    pub async fn send(&self, batch: ObservationBatch) -> Result<(), SinkError> {
        self.tx.send(batch).await.map_err(|_| SinkError::Closed)
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SinkError {
    #[error("batch sink closed (receiver dropped)")]
    Closed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::{CollectorTarget, IntegrationKind};
    use crate::observations::{ObservationBatch, ProbeResult, ProbeWindow};
    use crate::shared::types::{BitcoinNodeId, CollectorId, ObservationBatchId, SidecarId};
    use chrono::{Duration as ChronoDuration, Utc};
    use uuid::Uuid;

    struct DummyPoller {
        descriptor: CollectorDescriptor,
    }

    #[async_trait]
    impl PollingCollector for DummyPoller {
        fn descriptor(&self) -> &CollectorDescriptor {
            &self.descriptor
        }
        async fn poll(&self, _ctx: CollectionContext) -> ObservationBatch {
            let now = Utc::now();
            ObservationBatch {
                id: ObservationBatchId::new(),
                collector: self.descriptor.as_ref(),
                sidecar_id: SidecarId(Uuid::now_v7()),
                window: ProbeWindow::new(now, now).unwrap(),
                result: ProbeResult::Ok {
                    observations: vec![],
                },
            }
        }
    }

    fn dummy_descriptor() -> CollectorDescriptor {
        CollectorDescriptor {
            id: CollectorId("dummy".into()),
            integration: IntegrationKind::BitcoinCoreRpc {
                interval: ChronoDuration::seconds(10),
            },
            target: CollectorTarget::BitcoinNode(BitcoinNodeId("alice".into())),
            instance_label: "dummy".into(),
            description: None,
        }
    }

    #[test]
    fn polling_collector_is_object_safe() {
        let _boxed: Box<dyn PollingCollector> = Box::new(DummyPoller {
            descriptor: dummy_descriptor(),
        });
    }

    #[test]
    fn subscription_collector_is_object_safe() {
        struct DummySub {
            descriptor: CollectorDescriptor,
        }
        #[async_trait]
        impl SubscriptionCollector for DummySub {
            fn descriptor(&self) -> &CollectorDescriptor {
                &self.descriptor
            }
            async fn run(
                &self,
                _ctx: CollectionContext,
                _sink: BatchSink,
            ) -> Result<(), CollectionError> {
                Ok(())
            }
        }
        let _boxed: Box<dyn SubscriptionCollector> = Box::new(DummySub {
            descriptor: dummy_descriptor(),
        });
    }

    #[tokio::test]
    async fn batch_sink_send_returns_closed_after_receiver_drop() {
        let (tx, rx) = mpsc::channel(4);
        let sink = BatchSink::new(tx);
        drop(rx);

        let now = Utc::now();
        let batch = ObservationBatch {
            id: ObservationBatchId::new(),
            collector: dummy_descriptor().as_ref(),
            sidecar_id: SidecarId(Uuid::now_v7()),
            window: ProbeWindow::new(now, now).unwrap(),
            result: ProbeResult::Ok {
                observations: vec![],
            },
        };
        assert_eq!(sink.send(batch).await, Err(SinkError::Closed));
    }

    #[tokio::test]
    async fn batch_sink_send_succeeds_while_receiver_lives() {
        let (tx, mut rx) = mpsc::channel(4);
        let sink = BatchSink::new(tx);
        let now = Utc::now();
        let batch = ObservationBatch {
            id: ObservationBatchId::new(),
            collector: dummy_descriptor().as_ref(),
            sidecar_id: SidecarId(Uuid::now_v7()),
            window: ProbeWindow::new(now, now).unwrap(),
            result: ProbeResult::Ok {
                observations: vec![],
            },
        };
        sink.send(batch).await.expect("send ok");
        assert!(rx.recv().await.is_some());
    }
}
