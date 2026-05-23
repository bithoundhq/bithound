//! In-memory `ObservationStore` for tests.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::stream::{self, BoxStream, StreamExt};
use tokio::sync::Mutex;

use crate::observations::Observation;
use crate::storage::traits::{ObservationStore, StoreError};

#[derive(Default)]
pub struct MemoryObservationStore {
    inner: Mutex<Vec<Observation>>,
}

impl MemoryObservationStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ObservationStore for MemoryObservationStore {
    async fn append_many(&self, batch: &[Observation]) -> Result<(), StoreError> {
        let mut guard = self.inner.lock().await;
        guard.extend(batch.iter().cloned());
        Ok(())
    }

    async fn iter_since(
        &self,
        since: DateTime<Utc>,
    ) -> Result<BoxStream<'_, Result<Observation, StoreError>>, StoreError> {
        let guard = self.inner.lock().await;
        let mut snapshot: Vec<Observation> = guard
            .iter()
            .filter(|o| o.observed_at >= since)
            .cloned()
            .collect();
        snapshot.sort_by_key(|o| o.observed_at);
        Ok(stream::iter(snapshot.into_iter().map(Ok)).boxed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::{CollectorRef, IntegrationKind};
    use crate::observations::*;
    use crate::shared::types::*;
    use chrono::TimeZone;
    use futures::TryStreamExt;
    use std::collections::BTreeMap;

    fn obs_at(at: DateTime<Utc>) -> Observation {
        let ctx = ObservationContext {
            source: ObservationSource {
                sidecar_id: SidecarId(uuid::Uuid::now_v7()),
                collector: CollectorRef {
                    id: CollectorId("c".into()),
                    integration: IntegrationKind::BitcoinCoreRpc {
                        interval: chrono::Duration::seconds(10),
                    },
                    instance_label: "x".into(),
                },
            },
            subject: EntityRef::BitcoinNode(BitcoinNodeId("a".into())),
            observed_at: at,
            origin: ObservationOrigin::Collected,
        };
        Observation::event(
            ctx,
            crate::observations::EventName::parse("test.event").expect("valid"),
            EventSeverity::Info,
            None,
            Attributes(BTreeMap::new()),
        )
    }

    #[tokio::test]
    async fn append_and_iter_since() {
        let store = MemoryObservationStore::new();
        let t0 = Utc.timestamp_nanos(1_700_000_000_000_000_000);
        let t1 = t0 + chrono::Duration::seconds(10);
        let t2 = t0 + chrono::Duration::seconds(20);

        store
            .append_many(&[obs_at(t0), obs_at(t1), obs_at(t2)])
            .await
            .unwrap();

        let got: Vec<Observation> = store
            .iter_since(t1)
            .await
            .unwrap()
            .try_collect()
            .await
            .unwrap();
        assert_eq!(got.len(), 2);
        for o in &got {
            assert!(o.observed_at >= t1);
        }
    }

    #[tokio::test]
    async fn append_single_via_default_shim() {
        let store = MemoryObservationStore::new();
        let t0 = Utc.timestamp_nanos(1_700_000_000_000_000_000);
        store.append(&obs_at(t0)).await.unwrap();
        let got: Vec<Observation> = store
            .iter_since(t0 - chrono::Duration::seconds(1))
            .await
            .unwrap()
            .try_collect()
            .await
            .unwrap();
        assert_eq!(got.len(), 1);
    }
}
