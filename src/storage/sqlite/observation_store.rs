//! `SqliteObservationStore` per ADR-P1, ADR-P2 §P2.3.
//!
//! Hot fields (id, timestamps, subject, source, origin, payload kind) are
//! stored in indexed columns; full payload + attributes ride along as JSON
//! per ADR-P1 §13. The `integration` column also holds JSON so the
//! [`IntegrationKind`] interval survives round-trip — the column's
//! discriminant is implicit in the JSON tag and remains greppable.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::stream::{BoxStream, StreamExt, TryStreamExt};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use crate::collectors::{CollectorRef, IntegrationKind};
use crate::observations::{
    Attributes, Observation, ObservationOrigin, ObservationPayload, ObservationSource,
};
use crate::shared::types::*;
use crate::storage::traits::{ObservationStore, StoreError};

pub struct SqliteObservationStore {
    pool: SqlitePool,
}

impl SqliteObservationStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ObservationStore for SqliteObservationStore {
    async fn append_many(&self, batch: &[Observation]) -> Result<(), StoreError> {
        if batch.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin().await?;
        for obs in batch {
            let (subject_kind, subject_id) = subject_to_pair(&obs.subject);
            let integration_json = serde_json::to_string(&obs.source.collector.integration)?;
            let origin = origin_str(&obs.origin);
            let payload_kind = payload_kind(&obs.payload);
            let payload_json = serde_json::to_string(&obs.payload)?;
            let attributes_json = serde_json::to_string(&obs.attributes)?;
            sqlx::query(
                "INSERT INTO observations (\
                    id, observed_at, received_at, subject_kind, subject_id, \
                    sidecar_id, collector_id, integration, instance_label, \
                    origin, payload_kind, payload_json, attributes_json\
                 ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(obs.id.0)
            .bind(observed_to_nanos(obs.observed_at))
            .bind(obs.received_at.map(observed_to_nanos))
            .bind(subject_kind)
            .bind(subject_id)
            .bind(obs.source.sidecar_id.0)
            .bind(&obs.source.collector.id.0)
            .bind(integration_json)
            .bind(&obs.source.collector.instance_label)
            .bind(origin)
            .bind(payload_kind)
            .bind(payload_json)
            .bind(attributes_json)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn iter_since(
        &self,
        since: DateTime<Utc>,
    ) -> Result<BoxStream<'_, Result<Observation, StoreError>>, StoreError> {
        let since_nanos = observed_to_nanos(since);
        let stream = sqlx::query(
            "SELECT id, observed_at, received_at, subject_kind, subject_id, \
                    sidecar_id, collector_id, integration, instance_label, \
                    origin, payload_json, attributes_json \
             FROM observations \
             WHERE observed_at >= ? \
             ORDER BY observed_at ASC",
        )
        .bind(since_nanos)
        .fetch(&self.pool)
        .map(|row_res| -> Result<Observation, StoreError> {
            let row = row_res?;
            row_to_observation(&row)
        })
        .map_err(|e: StoreError| e)
        .into_stream();
        Ok(stream.boxed())
    }
}

fn observed_to_nanos(t: DateTime<Utc>) -> i64 {
    t.timestamp_nanos_opt()
        .expect("timestamp within i64 nanos range")
}

fn nanos_to_observed(n: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp_nanos(n)
}

fn subject_to_pair(subject: &EntityRef) -> (&'static str, &str) {
    match subject {
        EntityRef::Host(id) => ("host", id.0.as_str()),
        EntityRef::BitcoinNode(id) => ("bitcoin_node", id.0.as_str()),
        EntityRef::BitcoinPeer(id) => ("bitcoin_peer", id.0.as_str()),
        EntityRef::LndNode(id) => ("lnd_node", id.0.as_str()),
        EntityRef::LndPeer(id) => ("lnd_peer", id.0.as_str()),
        EntityRef::LndChannel(id) => ("lnd_channel", id.0.as_str()),
        EntityRef::LndInvoice(id) => ("lnd_invoice", id.0.as_str()),
    }
}

fn subject_from_pair(kind: &str, id: &str) -> Result<EntityRef, StoreError> {
    match kind {
        "host" => Ok(EntityRef::Host(HostId(id.to_string()))),
        "bitcoin_node" => Ok(EntityRef::BitcoinNode(BitcoinNodeId(id.to_string()))),
        "bitcoin_peer" => Ok(EntityRef::BitcoinPeer(BitcoinPeerId(id.to_string()))),
        "lnd_node" => Ok(EntityRef::LndNode(LndNodeId(id.to_string()))),
        "lnd_peer" => Ok(EntityRef::LndPeer(LndPeerId(id.to_string()))),
        "lnd_channel" => Ok(EntityRef::LndChannel(LndChannelId(id.to_string()))),
        "lnd_invoice" => Ok(EntityRef::LndInvoice(LndInvoiceId(id.to_string()))),
        other => Err(StoreError::Corruption(format!(
            "unknown subject_kind in observations row: {other}"
        ))),
    }
}

fn origin_str(origin: &ObservationOrigin) -> &'static str {
    match origin {
        ObservationOrigin::Collected => "collected",
        ObservationOrigin::Computed => "computed",
        ObservationOrigin::Imported => "imported",
        ObservationOrigin::UserReported => "user_reported",
    }
}

fn origin_from_str(s: &str) -> Result<ObservationOrigin, StoreError> {
    match s {
        "collected" => Ok(ObservationOrigin::Collected),
        "computed" => Ok(ObservationOrigin::Computed),
        "imported" => Ok(ObservationOrigin::Imported),
        "user_reported" => Ok(ObservationOrigin::UserReported),
        other => Err(StoreError::Corruption(format!(
            "unknown origin in observations row: {other}"
        ))),
    }
}

fn payload_kind(p: &ObservationPayload) -> &'static str {
    match p {
        ObservationPayload::Capability(_) => "capability",
        ObservationPayload::Diagnosis(_) => "diagnosis",
        ObservationPayload::Event(_) => "event",
        ObservationPayload::Heartbeat(_) => "heartbeat",
        ObservationPayload::Health(_) => "health",
        ObservationPayload::IncidentSignal(_) => "incident_signal",
        ObservationPayload::Inventory(_) => "inventory",
        ObservationPayload::Metric(_) => "metric",
        ObservationPayload::State(_) => "state",
        ObservationPayload::Transition(_) => "transition",
    }
}

fn row_to_observation(row: &sqlx::sqlite::SqliteRow) -> Result<Observation, StoreError> {
    let id: uuid::Uuid = row.try_get("id")?;
    let observed_at_nanos: i64 = row.try_get("observed_at")?;
    let received_at_nanos: Option<i64> = row.try_get("received_at")?;
    let subject_kind: String = row.try_get("subject_kind")?;
    let subject_id: String = row.try_get("subject_id")?;
    let sidecar_uuid: uuid::Uuid = row.try_get("sidecar_id")?;
    let collector_id: String = row.try_get("collector_id")?;
    let integration_json: String = row.try_get("integration")?;
    let instance_label: String = row.try_get("instance_label")?;
    let origin_text: String = row.try_get("origin")?;
    let payload_json: String = row.try_get("payload_json")?;
    let attributes_json: String = row.try_get("attributes_json")?;

    let subject = subject_from_pair(&subject_kind, &subject_id)?;
    let origin = origin_from_str(&origin_text)?;
    let integration: IntegrationKind = serde_json::from_str(&integration_json)?;
    let payload: ObservationPayload = serde_json::from_str(&payload_json)?;
    let attributes: Attributes = serde_json::from_str(&attributes_json)?;

    let source = ObservationSource {
        sidecar_id: SidecarId(sidecar_uuid),
        collector: CollectorRef {
            id: CollectorId(collector_id),
            integration,
            instance_label,
        },
    };

    Ok(Observation {
        id: ObservationId(id),
        observed_at: nanos_to_observed(observed_at_nanos),
        received_at: received_at_nanos.map(nanos_to_observed),
        source,
        subject,
        origin,
        attributes,
        payload,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::incidents::IncidentKind;
    use crate::observations::*;
    use crate::storage::sqlite::open_pool;
    use chrono::{Duration, TimeZone};
    use futures::TryStreamExt;
    use std::collections::BTreeMap;

    async fn fresh_store() -> (SqliteObservationStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let pool = open_pool(&dir.path().join("test.db"))
            .await
            .expect("open_pool");
        (SqliteObservationStore::new(pool), dir)
    }

    fn ctx(observed_at: DateTime<Utc>) -> ObservationContext {
        ObservationContext {
            source: ObservationSource {
                sidecar_id: SidecarId(uuid::Uuid::now_v7()),
                collector: CollectorRef {
                    id: CollectorId("test-collector".into()),
                    integration: IntegrationKind::BitcoinCoreRpc {
                        interval: Duration::seconds(10),
                    },
                    instance_label: "alice".into(),
                },
            },
            subject: EntityRef::BitcoinNode(BitcoinNodeId("alice".into())),
            observed_at,
            origin: ObservationOrigin::Collected,
        }
    }

    fn attrs() -> Attributes {
        let mut m = BTreeMap::new();
        m.insert("k".into(), AttributeValue::String("v".into()));
        Attributes(m)
    }

    /// One representative `Observation` for each of the ten payload variants.
    fn all_payload_variants(at: DateTime<Utc>) -> Vec<Observation> {
        let attrs = attrs;
        vec![
            Observation::metric(
                ctx(at),
                "btc.tip",
                MetricKind::Gauge,
                MetricValue::Numeric(NumericValue::I64(900_000)),
                Unit::Count,
                attrs(),
            ),
            Observation::capability(
                ctx(at),
                "bitcoin.rpc",
                CapabilityStatus::Available,
                None,
                attrs(),
            ),
            Observation::event(
                ctx(at),
                "bitcoin.test",
                EventSeverity::Info,
                Some("hello".into()),
                attrs(),
            ),
            Observation::heartbeat(
                ctx(at),
                1,
                at,
                Some(1000),
                "test-0.0.0",
                HeartbeatStatus::Alive,
                vec![],
                attrs(),
            ),
            Observation::health(
                ctx(at),
                "rpc",
                HealthStatus::Ok,
                Some(10),
                None,
                None,
                attrs(),
            ),
            Observation::inventory(ctx(at), "btc.peers", BTreeMap::new(), attrs()),
            Observation::state(
                ctx(at),
                StateObservation::BitcoinBlockchain(BitcoinBlockchainState {
                    chain: "test".into(),
                    blocks: 0,
                    headers: 0,
                    best_block_hash: None,
                    verification_progress: 1.0,
                    initial_block_download: false,
                    pruned: false,
                    size_on_disk_bytes: 0,
                }),
                attrs(),
            ),
            Observation::transition(
                ctx(at),
                "ibd.complete",
                StateAtom::String("in_ibd".into()),
                StateAtom::String("synced".into()),
                None,
                attrs(),
            ),
            {
                let kind = IncidentKind::parse("bitcoin.no_peers").expect("valid test kind");
                Observation::incident_signal(
                    ctx(at),
                    IncidentSignalObservation {
                        signal: SignalName::for_incident_kind(&kind),
                        incident_kind: kind,
                        severity: SignalSeverity::Critical,
                        status: SignalStatus::Active,
                        confidence: Confidence::High,
                        evidence: vec![],
                    },
                    attrs(),
                )
            },
            Observation::diagnosis(
                ctx(at),
                DiagnosisObservation {
                    diagnosis: DiagnosisName("bitcoin.tip_lag.assessment".into()),
                    summary: "stuck".into(),
                    confidence: Confidence::Medium,
                    likely_causes: vec![],
                    recommended_actions: vec![],
                    evidence: vec![],
                },
                attrs(),
            ),
        ]
    }

    #[tokio::test]
    async fn round_trip_all_payload_variants() {
        let (store, _dir) = fresh_store().await;
        let at = Utc.timestamp_nanos(1_700_000_000_000_000_000);
        let batch = all_payload_variants(at);
        store.append_many(&batch).await.expect("append_many");

        let mut observed: Vec<Observation> = store
            .iter_since(at - Duration::seconds(1))
            .await
            .expect("iter_since")
            .try_collect()
            .await
            .expect("collect");

        assert_eq!(observed.len(), batch.len());
        // Rows share `observed_at`; sort both sides by id for stable comparison.
        observed.sort_by_key(|o| o.id.0);
        let mut expected = batch;
        expected.sort_by_key(|o| o.id.0);
        for (got, expected) in observed.iter().zip(expected.iter()) {
            let got_json = serde_json::to_value(got).unwrap();
            let expected_json = serde_json::to_value(expected).unwrap();
            assert_eq!(got_json, expected_json);
        }
    }

    #[tokio::test]
    async fn iter_since_respects_filter() {
        let (store, _dir) = fresh_store().await;
        let t_old = Utc.timestamp_nanos(1_700_000_000_000_000_000);
        let t_new = t_old + Duration::seconds(60);

        let mut old_obs = all_payload_variants(t_old);
        let new_obs = all_payload_variants(t_new);
        old_obs.extend(new_obs);
        store.append_many(&old_obs).await.expect("append");

        let after_mid: Vec<Observation> = store
            .iter_since(t_old + Duration::seconds(30))
            .await
            .expect("iter")
            .try_collect()
            .await
            .expect("collect");

        assert_eq!(after_mid.len(), 10);
        for o in &after_mid {
            assert!(o.observed_at >= t_old + Duration::seconds(30));
        }
    }

    #[tokio::test]
    async fn concurrent_append_many_serializes_under_wal() {
        let (store, _dir) = fresh_store().await;
        let at = Utc.timestamp_nanos(1_700_000_000_000_000_000);
        let store = std::sync::Arc::new(store);

        let mut tasks = Vec::new();
        for i in 0..8 {
            let store = store.clone();
            let at_i = at + Duration::seconds(i);
            tasks.push(tokio::spawn(async move {
                let batch = all_payload_variants(at_i);
                store.append_many(&batch).await
            }));
        }
        for t in tasks {
            t.await.expect("join").expect("append");
        }

        let all: Vec<Observation> = store
            .iter_since(at - Duration::seconds(1))
            .await
            .expect("iter")
            .try_collect()
            .await
            .expect("collect");
        assert_eq!(all.len(), 8 * 10);
    }

    #[tokio::test]
    async fn append_empty_batch_is_noop() {
        let (store, _dir) = fresh_store().await;
        store.append_many(&[]).await.expect("noop ok");
    }
}
