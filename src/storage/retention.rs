//! Retention background task per ADR-P2 §P2.5 (with `attempts_max_age`
//! added in ADR-P3 §P3.11).
//!
//! The task ticks on `vacuum_interval` and deletes rows older than the
//! configured ages from each table, then runs `VACUUM`. A `None` age
//! disables the sweep for that table.

use std::time::Duration;

use chrono::Utc;
use sqlx::sqlite::SqlitePool;
use tokio::sync::broadcast;
use tokio::time::MissedTickBehavior;

#[derive(Debug, Clone)]
pub struct RetentionConfig {
    pub observations_max_age: Option<Duration>,
    pub incidents_max_age: Option<Duration>,
    pub suppressions_grace: Option<Duration>,
    /// Per ADR-P3 §P3.11. Defaults shorter than observations because
    /// attempts are denser (one row per dispatch + retries).
    pub attempts_max_age: Option<Duration>,
    pub vacuum_interval: Duration,
}

/// Run the retention loop until `shutdown` fires.
pub async fn run(pool: SqlitePool, config: RetentionConfig, mut shutdown: broadcast::Receiver<()>) {
    let mut ticker = tokio::time::interval(config.vacuum_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    // First tick fires immediately; we want to wait one interval before the
    // first sweep so cold-start doesn't VACUUM a fresh DB.
    ticker.tick().await;

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                sweep(&pool, &config).await;
            }
            _ = shutdown.recv() => break,
        }
    }
}

/// One pass over each table. Errors are logged via `tracing::warn!` and
/// otherwise ignored — retention is best-effort.
pub async fn sweep(pool: &SqlitePool, config: &RetentionConfig) {
    let now_nanos = Utc::now()
        .timestamp_nanos_opt()
        .expect("system time within i64 nanos range");

    if let Some(age) = config.observations_max_age {
        let cutoff = now_nanos - duration_nanos(age);
        if let Err(e) = sqlx::query("DELETE FROM observations WHERE observed_at < ?")
            .bind(cutoff)
            .execute(pool)
            .await
        {
            tracing::warn!(error = %e, "retention: observations sweep failed");
        }
    }
    if let Some(age) = config.incidents_max_age {
        let cutoff = now_nanos - duration_nanos(age);
        if let Err(e) =
            sqlx::query("DELETE FROM incidents WHERE resolved_at IS NOT NULL AND resolved_at < ?")
                .bind(cutoff)
                .execute(pool)
                .await
        {
            tracing::warn!(error = %e, "retention: incidents sweep failed");
        }
    }
    if let Some(grace) = config.suppressions_grace {
        let cutoff = now_nanos - duration_nanos(grace);
        if let Err(e) =
            sqlx::query("DELETE FROM suppression_rules WHERE until IS NOT NULL AND until < ?")
                .bind(cutoff)
                .execute(pool)
                .await
        {
            tracing::warn!(error = %e, "retention: suppression_rules sweep failed");
        }
    }
    if let Some(age) = config.attempts_max_age {
        let cutoff = now_nanos - duration_nanos(age);
        // The notification_attempts table is added by BTH-52; until then the
        // query errors and is logged at debug level. The status guard
        // protects in-flight rows once the table lands.
        match sqlx::query(
            "DELETE FROM notification_attempts WHERE attempted_at < ? AND status != 'Pending'",
        )
        .bind(cutoff)
        .execute(pool)
        .await
        {
            Ok(_) => {}
            Err(e) => {
                tracing::debug!(error = %e, "retention: notification_attempts sweep skipped");
            }
        }
    }

    if let Err(e) = sqlx::query("VACUUM").execute(pool).await {
        tracing::warn!(error = %e, "retention: VACUUM failed");
    }
}

fn duration_nanos(d: Duration) -> i64 {
    i64::try_from(d.as_nanos()).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::{CollectorRef, IntegrationKind};
    use crate::incidents::repository::IncidentRepository;
    use crate::incidents::{
        Incident, IncidentFingerprint, IncidentKind, IncidentSeverity, IncidentStatus,
    };
    use crate::observations::*;
    use crate::shared::types::*;
    use crate::storage::sqlite::incident_repository::SqliteIncidentRepository;
    use crate::storage::sqlite::observation_store::SqliteObservationStore;
    use crate::storage::sqlite::open_pool;
    use crate::storage::traits::ObservationStore;
    use chrono::TimeZone;

    fn obs_at(at: chrono::DateTime<Utc>) -> Observation {
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
            "test",
            EventSeverity::Info,
            None,
            Attributes(std::collections::BTreeMap::new()),
        )
    }

    fn resolved_incident_at(at: chrono::DateTime<Utc>) -> Incident {
        let subject = EntityRef::BitcoinNode(BitcoinNodeId("a".into()));
        let kind = IncidentKind("bitcoin.no_peers".into());
        Incident {
            id: IncidentId::new(),
            fingerprint: IncidentFingerprint {
                subject: subject.clone(),
                kind: kind.clone(),
                dimension: None,
            },
            kind,
            subject,
            severity: IncidentSeverity::Critical,
            status: IncidentStatus::Resolved,
            opened_at: at,
            updated_at: at,
            resolved_at: Some(at),
            signal_observation_ids: vec![],
            evidence: vec![],
            summary: "x".into(),
            evidence_summary: vec![],
        }
    }

    #[tokio::test]
    async fn sweep_deletes_old_observations() {
        let dir = tempfile::tempdir().unwrap();
        let pool = open_pool(&dir.path().join("test.db")).await.unwrap();
        let store = SqliteObservationStore::new(pool.clone());

        let now = Utc::now();
        let old = now - chrono::Duration::hours(10);
        let recent = now - chrono::Duration::minutes(1);
        store
            .append_many(&[obs_at(old), obs_at(recent)])
            .await
            .unwrap();

        let cfg = RetentionConfig {
            observations_max_age: Some(Duration::from_secs(3600)),
            incidents_max_age: None,
            suppressions_grace: None,
            attempts_max_age: None,
            vacuum_interval: Duration::from_secs(3600),
        };
        sweep(&pool, &cfg).await;

        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM observations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(remaining, 1, "old observation pruned, recent kept");
    }

    #[tokio::test]
    async fn sweep_deletes_old_resolved_incidents() {
        let dir = tempfile::tempdir().unwrap();
        let pool = open_pool(&dir.path().join("test.db")).await.unwrap();
        let repo = SqliteIncidentRepository::new(pool.clone());

        let old = Utc.timestamp_nanos(1_000_000_000_000_000_000);
        let recent = Utc::now() - chrono::Duration::minutes(1);
        repo.save(&resolved_incident_at(old)).await.unwrap();
        repo.save(&resolved_incident_at(recent)).await.unwrap();

        let cfg = RetentionConfig {
            observations_max_age: None,
            incidents_max_age: Some(Duration::from_secs(3600)),
            suppressions_grace: None,
            attempts_max_age: None,
            vacuum_interval: Duration::from_secs(3600),
        };
        sweep(&pool, &cfg).await;

        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM incidents")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(remaining, 1);
    }

    #[tokio::test]
    async fn none_ages_disable_retention() {
        let dir = tempfile::tempdir().unwrap();
        let pool = open_pool(&dir.path().join("test.db")).await.unwrap();
        let store = SqliteObservationStore::new(pool.clone());
        let very_old = Utc.timestamp_nanos(1_000_000_000_000_000_000);
        store.append_many(&[obs_at(very_old)]).await.unwrap();

        let cfg = RetentionConfig {
            observations_max_age: None,
            incidents_max_age: None,
            suppressions_grace: None,
            attempts_max_age: None,
            vacuum_interval: Duration::from_secs(3600),
        };
        sweep(&pool, &cfg).await;

        let remaining: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM observations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(remaining, 1, "None disables sweep");
    }

    #[tokio::test]
    async fn run_exits_on_shutdown() {
        let dir = tempfile::tempdir().unwrap();
        let pool = open_pool(&dir.path().join("test.db")).await.unwrap();

        let cfg = RetentionConfig {
            observations_max_age: None,
            incidents_max_age: None,
            suppressions_grace: None,
            attempts_max_age: None,
            vacuum_interval: Duration::from_millis(50),
        };
        let (tx, rx) = broadcast::channel(1);
        let handle = tokio::spawn(run(pool, cfg, rx));

        // Let it tick at least once.
        tokio::time::sleep(Duration::from_millis(80)).await;
        tx.send(()).unwrap();
        let res = tokio::time::timeout(Duration::from_secs(1), handle).await;
        assert!(res.is_ok(), "loop must exit within 1s of shutdown");
    }
}
