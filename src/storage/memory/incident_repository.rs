//! In-memory `IncidentRepository` for tests.

use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::Mutex;

use crate::incidents::repository::{IncidentRepository, RepoError};
use crate::incidents::{Incident, IncidentStatus};
use crate::shared::types::IncidentId;

#[derive(Default)]
pub struct MemoryIncidentRepository {
    inner: Mutex<HashMap<IncidentId, Incident>>,
}

impl MemoryIncidentRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl IncidentRepository for MemoryIncidentRepository {
    async fn load_open(&self) -> Result<Vec<Incident>, RepoError> {
        let guard = self.inner.lock().await;
        let mut out: Vec<Incident> = guard
            .values()
            .filter(|i| i.status != IncidentStatus::Resolved)
            .cloned()
            .collect();
        out.sort_by_key(|i| i.opened_at);
        Ok(out)
    }

    async fn save(&self, incident: &Incident) -> Result<(), RepoError> {
        let mut guard = self.inner.lock().await;
        guard.insert(incident.id.clone(), incident.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::incidents::{
        Incident, IncidentFingerprint, IncidentKind, IncidentSeverity, IncidentStatus,
    };
    use crate::shared::types::*;
    use chrono::{TimeZone, Utc};

    fn incident(status: IncidentStatus) -> Incident {
        let at = Utc.timestamp_nanos(1_700_000_000_000_000_000);
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
            status,
            opened_at: at,
            updated_at: at,
            resolved_at: None,
            signal_observation_ids: vec![],
            evidence: vec![],
            summary: "x".into(),
            evidence_summary: vec![],
        }
    }

    #[tokio::test]
    async fn save_and_load_open_excludes_resolved() {
        let repo = MemoryIncidentRepository::new();
        let open = incident(IncidentStatus::Open);
        let ack = incident(IncidentStatus::Acknowledged);
        let resolved = incident(IncidentStatus::Resolved);
        for i in [&open, &ack, &resolved] {
            repo.save(i).await.unwrap();
        }
        let loaded = repo.load_open().await.unwrap();
        assert_eq!(loaded.len(), 2);
        let ids: Vec<_> = loaded.iter().map(|i| i.id.clone()).collect();
        assert!(ids.contains(&open.id));
        assert!(ids.contains(&ack.id));
        assert!(!ids.contains(&resolved.id));
    }

    #[tokio::test]
    async fn save_replaces_on_same_id() {
        let repo = MemoryIncidentRepository::new();
        let mut inc = incident(IncidentStatus::Open);
        repo.save(&inc).await.unwrap();
        inc.summary = "updated".into();
        repo.save(&inc).await.unwrap();
        let loaded = repo.load_open().await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].summary, "updated");
    }
}
