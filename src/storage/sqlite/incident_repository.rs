//! `SqliteIncidentRepository` per ADR-P2 §P2.2.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;

use crate::incidents::repository::{IncidentRepository, RepoError};
use crate::incidents::{Incident, IncidentSeverity, IncidentStatus};
use crate::shared::types::*;

pub struct SqliteIncidentRepository {
    pool: SqlitePool,
}

impl SqliteIncidentRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl IncidentRepository for SqliteIncidentRepository {
    async fn load_open(&self) -> Result<Vec<Incident>, RepoError> {
        let rows = sqlx::query(
            "SELECT incident_json FROM incidents WHERE status != ? ORDER BY opened_at ASC",
        )
        .bind(status_str(&IncidentStatus::Resolved))
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let json: String = row.try_get("incident_json")?;
            let incident: Incident = serde_json::from_str(&json)?;
            out.push(incident);
        }
        Ok(out)
    }

    async fn save(&self, incident: &Incident) -> Result<(), RepoError> {
        let (subject_kind, subject_id) = subject_to_pair(&incident.subject);
        let incident_json = serde_json::to_string(incident)?;
        sqlx::query(
            "INSERT INTO incidents (\
                id, fingerprint, kind, subject_kind, subject_id, severity, status, \
                opened_at, updated_at, resolved_at, incident_json\
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET \
                fingerprint   = excluded.fingerprint, \
                kind          = excluded.kind, \
                subject_kind  = excluded.subject_kind, \
                subject_id    = excluded.subject_id, \
                severity      = excluded.severity, \
                status        = excluded.status, \
                opened_at     = excluded.opened_at, \
                updated_at    = excluded.updated_at, \
                resolved_at   = excluded.resolved_at, \
                incident_json = excluded.incident_json",
        )
        .bind(incident.id.0)
        .bind(incident.fingerprint.as_key())
        .bind(incident.kind.as_str())
        .bind(subject_kind)
        .bind(subject_id)
        .bind(severity_str(&incident.severity))
        .bind(status_str(&incident.status))
        .bind(nanos(incident.opened_at))
        .bind(nanos(incident.updated_at))
        .bind(incident.resolved_at.map(nanos))
        .bind(incident_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

fn nanos(t: DateTime<Utc>) -> i64 {
    t.timestamp_nanos_opt()
        .expect("timestamp within i64 nanos range")
}

fn severity_str(s: &IncidentSeverity) -> &'static str {
    match s {
        IncidentSeverity::Info => "Info",
        IncidentSeverity::Warning => "Warning",
        IncidentSeverity::Critical => "Critical",
    }
}

fn status_str(s: &IncidentStatus) -> &'static str {
    match s {
        IncidentStatus::Open => "Open",
        IncidentStatus::Acknowledged => "Acknowledged",
        IncidentStatus::Resolved => "Resolved",
        IncidentStatus::Suppressed => "Suppressed",
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::incidents::{IncidentFingerprint, IncidentKind};
    use crate::storage::sqlite::open_pool;
    use chrono::TimeZone;

    async fn fresh_repo() -> (SqliteIncidentRepository, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let pool = open_pool(&dir.path().join("test.db"))
            .await
            .expect("open_pool");
        (SqliteIncidentRepository::new(pool), dir)
    }

    fn sample_incident(status: IncidentStatus) -> Incident {
        let opened = Utc.timestamp_nanos(1_700_000_000_000_000_000);
        let subject = EntityRef::BitcoinNode(BitcoinNodeId("alice".into()));
        let kind = IncidentKind::parse("bitcoin.no_peers").expect("valid test kind");
        let resolved_at = matches!(status, IncidentStatus::Resolved).then_some(opened);
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
            status,
            opened_at: opened,
            updated_at: opened,
            resolved_at,
            signal_observation_ids: vec![],
            evidence: vec![],
            summary: "no peers".into(),
            evidence_summary: vec![],
        }
    }

    #[tokio::test]
    async fn save_then_load_open_returns_non_resolved() {
        let (repo, _dir) = fresh_repo().await;
        let open = sample_incident(IncidentStatus::Open);
        let ack = sample_incident(IncidentStatus::Acknowledged);
        let supp = sample_incident(IncidentStatus::Suppressed);
        let resolved = sample_incident(IncidentStatus::Resolved);
        for inc in [&open, &ack, &supp, &resolved] {
            repo.save(inc).await.expect("save");
        }
        let loaded = repo.load_open().await.expect("load_open");
        assert_eq!(loaded.len(), 3, "Resolved must be excluded");
        let ids: Vec<_> = loaded.iter().map(|i| i.id.clone()).collect();
        assert!(ids.contains(&open.id));
        assert!(ids.contains(&ack.id));
        assert!(ids.contains(&supp.id));
        assert!(!ids.contains(&resolved.id));
    }

    #[tokio::test]
    async fn save_replaces_existing_row_on_id_conflict() {
        let (repo, _dir) = fresh_repo().await;
        let mut inc = sample_incident(IncidentStatus::Open);
        repo.save(&inc).await.expect("first save");

        inc.summary = "updated".into();
        inc.status = IncidentStatus::Acknowledged;
        repo.save(&inc).await.expect("second save");

        let loaded = repo.load_open().await.expect("load");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, inc.id);
        assert_eq!(loaded[0].summary, "updated");
        assert_eq!(loaded[0].status, IncidentStatus::Acknowledged);
    }

    #[tokio::test]
    async fn round_trip_all_status_variants() {
        for status in [
            IncidentStatus::Open,
            IncidentStatus::Acknowledged,
            IncidentStatus::Resolved,
            IncidentStatus::Suppressed,
        ] {
            let (repo, _dir) = fresh_repo().await;
            let inc = sample_incident(status.clone());
            repo.save(&inc).await.expect("save");

            // For Resolved we can't see it via load_open; instead query directly.
            let row = sqlx::query("SELECT incident_json FROM incidents WHERE id = ?")
                .bind(inc.id.0)
                .fetch_one(&repo.pool)
                .await
                .expect("fetch");
            let json: String = row.try_get("incident_json").expect("col");
            let got: Incident = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(got.status, status);
        }
    }
}
