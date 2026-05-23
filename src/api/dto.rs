//! Wire DTOs for the operator HTTP API.
//!
//! Kept separate from the domain types so the wire format can evolve
//! cautiously — domain shape changes don't automatically break
//! operator-facing JSON.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::incidents::{Incident, IncidentSeverity, IncidentStatus};
use crate::observations::Observation;
use crate::shared::types::{EntityRef, IncidentId, SidecarId};

// ───── /health ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthDto {
    pub sidecar_id: Uuid,
    pub version: String,
    pub uptime_seconds: u64,
    pub latest_heartbeat_at: Option<DateTime<Utc>>,
    pub db: DbHealthDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbHealthDto {
    pub reachable: bool,
    pub latency_ms: Option<u64>,
}

// ───── /incidents/open ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentListDto {
    pub count: usize,
    pub incidents: Vec<IncidentSummaryDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentSummaryDto {
    pub id: Uuid,
    pub fingerprint: String,
    pub kind: String,
    pub subject: SubjectDto,
    pub severity: IncidentSeverity,
    pub status: IncidentStatus,
    pub opened_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub summary: String,
}

impl From<&Incident> for IncidentSummaryDto {
    fn from(i: &Incident) -> Self {
        IncidentSummaryDto {
            id: i.id.0,
            fingerprint: i.fingerprint.as_key(),
            kind: i.kind.as_str().to_string(),
            subject: SubjectDto::from(&i.subject),
            severity: i.severity.clone(),
            status: i.status.clone(),
            opened_at: i.opened_at,
            updated_at: i.updated_at,
            summary: i.summary.clone(),
        }
    }
}

// ───── /incidents/:id ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentDetailDto {
    pub id: Uuid,
    pub fingerprint: String,
    pub kind: String,
    pub subject: SubjectDto,
    pub severity: IncidentSeverity,
    pub status: IncidentStatus,
    pub opened_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub signal_observation_ids: Vec<Uuid>,
    pub evidence: Vec<Uuid>,
    pub summary: String,
    pub evidence_summary: Vec<String>,
}

impl From<&Incident> for IncidentDetailDto {
    fn from(i: &Incident) -> Self {
        IncidentDetailDto {
            id: i.id.0,
            fingerprint: i.fingerprint.as_key(),
            kind: i.kind.as_str().to_string(),
            subject: SubjectDto::from(&i.subject),
            severity: i.severity.clone(),
            status: i.status.clone(),
            opened_at: i.opened_at,
            updated_at: i.updated_at,
            resolved_at: i.resolved_at,
            signal_observation_ids: i.signal_observation_ids.iter().map(|o| o.0).collect(),
            evidence: i.evidence.iter().map(|e| e.0 .0).collect(),
            summary: i.summary.clone(),
            evidence_summary: i.evidence_summary.clone(),
        }
    }
}

// ───── /incidents/:id/evidence ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentEvidenceDto {
    pub incident_id: Uuid,
    pub evidence: Vec<EvidenceObservationDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceObservationDto {
    pub observation_id: Uuid,
    pub observed_at: DateTime<Utc>,
    pub subject: SubjectDto,
    pub source: EvidenceSourceDto,
    pub origin: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceSourceDto {
    pub collector_id: String,
    pub sidecar_id: Uuid,
}

impl EvidenceObservationDto {
    pub fn from_observation(obs: &Observation) -> Result<Self, serde_json::Error> {
        let payload = serde_json::to_value(&obs.payload)?;
        Ok(EvidenceObservationDto {
            observation_id: obs.id.0,
            observed_at: obs.observed_at,
            subject: SubjectDto::from(&obs.subject),
            source: EvidenceSourceDto {
                collector_id: obs.source.collector.id.0.clone(),
                sidecar_id: obs.source.sidecar_id.0,
            },
            origin: format!("{:?}", obs.origin),
            payload,
        })
    }
}

// ───── Helpers ────────────────────────────────────────────────────────

/// Operator-facing rendering of an `EntityRef`. Scoped sub-entity
/// variants get both ids in the response so the operator can see
/// which parent node a peer / channel / invoice belongs to.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum SubjectDto {
    Sidecar { id: Uuid },
    Host { id: String },
    BitcoinNode { id: String },
    BitcoinPeer { node_id: String, peer_id: String },
    LndNode { id: String },
    LndPeer { node_id: String, peer_id: String },
    LndChannel { node_id: String, channel_id: String },
    LndInvoice { node_id: String, invoice_id: String },
}

impl From<&EntityRef> for SubjectDto {
    fn from(e: &EntityRef) -> Self {
        match e {
            EntityRef::Sidecar(SidecarId(id)) => SubjectDto::Sidecar { id: *id },
            EntityRef::Host(id) => SubjectDto::Host { id: id.0.clone() },
            EntityRef::BitcoinNode(id) => SubjectDto::BitcoinNode { id: id.0.clone() },
            EntityRef::BitcoinPeer { node_id, peer_id } => SubjectDto::BitcoinPeer {
                node_id: node_id.0.clone(),
                peer_id: peer_id.0.clone(),
            },
            EntityRef::LndNode(id) => SubjectDto::LndNode { id: id.0.clone() },
            EntityRef::LndPeer { node_id, peer_id } => SubjectDto::LndPeer {
                node_id: node_id.0.clone(),
                peer_id: peer_id.0.clone(),
            },
            EntityRef::LndChannel {
                node_id,
                channel_id,
            } => SubjectDto::LndChannel {
                node_id: node_id.0.clone(),
                channel_id: channel_id.0.clone(),
            },
            EntityRef::LndInvoice {
                node_id,
                invoice_id,
            } => SubjectDto::LndInvoice {
                node_id: node_id.0.clone(),
                invoice_id: invoice_id.0.clone(),
            },
        }
    }
}

/// Parse a path-param `:id` string into a typed `IncidentId`. Used
/// by the incident handlers so handler code doesn't need to depend
/// on `uuid` directly.
pub fn parse_incident_id(s: &str) -> Result<IncidentId, uuid::Error> {
    Uuid::parse_str(s).map(IncidentId)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observations::{Attributes, EventName, EventSeverity, ObservationContext};
    use crate::observations::{ObservationOrigin, ObservationSource};
    use crate::shared::types::{
        BitcoinNodeId, BitcoinPeerId, CollectorId, HostId, LndChannelId, LndInvoiceId, LndNodeId,
        LndPeerId,
    };
    use std::collections::BTreeMap;

    /// DTOs must round-trip via serde — operators feeding `curl` output
    /// back into a script expect the JSON to parse cleanly back into
    /// the same shape.
    #[test]
    fn incident_summary_dto_round_trips_via_serde() {
        let dto = IncidentSummaryDto {
            id: Uuid::now_v7(),
            fingerprint: "bitcoin_node|alice|bitcoin.no_peers|-".into(),
            kind: "bitcoin.no_peers".into(),
            subject: SubjectDto::BitcoinNode { id: "alice".into() },
            severity: IncidentSeverity::Critical,
            status: IncidentStatus::Open,
            opened_at: Utc::now(),
            updated_at: Utc::now(),
            summary: "no peers".into(),
        };
        let json = serde_json::to_string(&dto).unwrap();
        let back: IncidentSummaryDto = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, dto.id);
        assert_eq!(back.fingerprint, dto.fingerprint);
        assert_eq!(back.severity, dto.severity);
    }

    /// Scoped sub-entity subjects must serialize both the parent and
    /// the local id — that's how the operator distinguishes
    /// `lnd-a/chan-1` from `lnd-b/chan-1`.
    #[test]
    fn subject_dto_renders_scoped_lnd_channel() {
        let dto: SubjectDto = (&EntityRef::LndChannel {
            node_id: LndNodeId("ln-a".into()),
            channel_id: LndChannelId("123:1:0".into()),
        })
            .into();
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("\"type\":\"lnd_channel\""));
        assert!(json.contains("\"node_id\":\"ln-a\""));
        assert!(json.contains("\"channel_id\":\"123:1:0\""));

        let back: SubjectDto = serde_json::from_str(&json).unwrap();
        match back {
            SubjectDto::LndChannel {
                node_id,
                channel_id,
            } => {
                assert_eq!(node_id, "ln-a");
                assert_eq!(channel_id, "123:1:0");
            }
            other => panic!("unexpected SubjectDto: {other:?}"),
        }
    }

    #[test]
    fn subject_dto_covers_every_entity_ref_variant() {
        let variants = [
            EntityRef::Sidecar(SidecarId(Uuid::nil())),
            EntityRef::Host(HostId("h".into())),
            EntityRef::BitcoinNode(BitcoinNodeId("n".into())),
            EntityRef::BitcoinPeer {
                node_id: BitcoinNodeId("n".into()),
                peer_id: BitcoinPeerId("p".into()),
            },
            EntityRef::LndNode(LndNodeId("ln".into())),
            EntityRef::LndPeer {
                node_id: LndNodeId("ln".into()),
                peer_id: LndPeerId("lp".into()),
            },
            EntityRef::LndChannel {
                node_id: LndNodeId("ln".into()),
                channel_id: LndChannelId("lc".into()),
            },
            EntityRef::LndInvoice {
                node_id: LndNodeId("ln".into()),
                invoice_id: LndInvoiceId("li".into()),
            },
        ];
        for v in &variants {
            let dto: SubjectDto = v.into();
            let json = serde_json::to_string(&dto).expect("serialize");
            let _back: SubjectDto = serde_json::from_str(&json).expect("deserialize");
        }
    }

    #[test]
    fn evidence_observation_dto_from_observation() {
        let ctx = ObservationContext {
            source: ObservationSource {
                sidecar_id: SidecarId(Uuid::now_v7()),
                collector: crate::collectors::CollectorRef {
                    id: CollectorId("c".into()),
                    integration: crate::collectors::IntegrationKind::BitcoinCoreRpc {
                        interval: chrono::Duration::seconds(10),
                    },
                    instance_label: "x".into(),
                },
            },
            subject: EntityRef::BitcoinNode(BitcoinNodeId("alice".into())),
            observed_at: Utc::now(),
            origin: ObservationOrigin::Collected,
        };
        let obs = Observation::event(
            ctx,
            EventName::parse("bitcoin.event").expect("valid"),
            EventSeverity::Info,
            None,
            Attributes(BTreeMap::new()),
        );
        let dto = EvidenceObservationDto::from_observation(&obs).expect("convert");
        assert_eq!(dto.observation_id, obs.id.0);
        assert_eq!(dto.source.collector_id, "c");
    }
}
