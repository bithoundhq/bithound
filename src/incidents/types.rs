use serde::{Deserialize, Serialize};

use chrono::{DateTime, Utc};

use crate::shared::types::{EntityRef, EvidenceRef, IncidentId, ObservationId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Incident {
    pub id: IncidentId,
    pub fingerprint: IncidentFingerprint,
    pub kind: IncidentKind,
    pub subject: EntityRef,

    pub severity: IncidentSeverity,
    pub status: IncidentStatus,

    pub opened_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,

    pub signal_observation_ids: Vec<ObservationId>,
    pub evidence: Vec<EvidenceRef>,

    pub summary: String,

    /// Optional durable display copy for retention purposes.
    pub evidence_summary: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IncidentKind(pub String);

impl IncidentKind {
    /// Lift a canonical name from [`crate::incidents::well_known`] into
    /// a typed `IncidentKind`. Rules use this to reference the kind
    /// they emit signals for without re-typing the string literal —
    /// drift between the constant and `default_kinds.toml` is caught
    /// by the parity test in `well_known`.
    pub fn from_well_known(name: &'static str) -> Self {
        IncidentKind(name.to_string())
    }
}

/// Structured primary key for an incident.
///
/// The engine computes this from`(draft.subject, draft.kind, draft.dimension)`
/// on receipt and uses it as the lookup key for open incidents.
/// `as_key` returns a stable string form for storage indexing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IncidentFingerprint {
    pub subject: EntityRef,
    pub kind: IncidentKind,
    pub dimension: Option<String>,
}

impl IncidentFingerprint {
    /// Stable string form for storage indexing.
    ///
    /// Format: `"<subject_kind>|<subject_id>|<incident_kind>|<dimension or '-'>"`.
    pub fn as_key(&self) -> String {
        let (subject_kind, subject_id) = subject_kind_and_id(&self.subject);
        let dim = self.dimension.as_deref().unwrap_or("-");
        format!("{}|{}|{}|{}", subject_kind, subject_id, self.kind.0, dim)
    }
}

fn subject_kind_and_id(subject: &EntityRef) -> (&'static str, &str) {
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
    use crate::shared::types::BitcoinNodeId;

    fn fp_btc(kind: &str, dim: Option<&str>) -> IncidentFingerprint {
        IncidentFingerprint {
            subject: EntityRef::BitcoinNode(BitcoinNodeId("alice".into())),
            kind: IncidentKind(kind.into()),
            dimension: dim.map(|s| s.to_string()),
        }
    }

    #[test]
    fn fingerprint_equality_is_structural() {
        let a = fp_btc("bitcoin.tip_lag", None);
        let b = fp_btc("bitcoin.tip_lag", None);
        let c = fp_btc("bitcoin.peer_starvation", None);
        let d = fp_btc("bitcoin.tip_lag", Some("aux"));
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
    }

    #[test]
    fn as_key_format_no_dimension() {
        let fp = fp_btc("bitcoin.tip_lag", None);
        assert_eq!(fp.as_key(), "bitcoin_node|alice|bitcoin.tip_lag|-");
    }

    #[test]
    fn as_key_format_with_dimension() {
        let fp = fp_btc("host.disk_exhaustion", Some("/var/lib/bitcoin"));
        assert_eq!(
            fp.as_key(),
            "bitcoin_node|alice|host.disk_exhaustion|/var/lib/bitcoin"
        );
    }

    #[test]
    fn as_key_is_stable_across_equal_inputs() {
        let a = fp_btc("bitcoin.tip_lag", Some("x"));
        let b = fp_btc("bitcoin.tip_lag", Some("x"));
        assert_eq!(a.as_key(), b.as_key());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IncidentSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IncidentStatus {
    Open,
    Acknowledged,
    Resolved,
    /// Reserved for V0.2 — operator-acknowledged-known. Not set by the V0/V0.1 engine.
    /// V0.1 suppression is notifier-side via `SuppressionRule`; see ADR-L5.
    Suppressed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IncidentNotificationEventKind {
    Opened,
    Escalated,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IncidentLifecycleEvent {
    Opened(Incident),
    Escalated {
        incident: Incident,
        previous_severity: IncidentSeverity,
        new_severity: IncidentSeverity,
    },
    Resolved(Incident),
}

impl IncidentLifecycleEvent {
    pub fn notification_kind(&self) -> IncidentNotificationEventKind {
        match self {
            IncidentLifecycleEvent::Opened(_) => IncidentNotificationEventKind::Opened,
            IncidentLifecycleEvent::Escalated { .. } => IncidentNotificationEventKind::Escalated,
            IncidentLifecycleEvent::Resolved(_) => IncidentNotificationEventKind::Resolved,
        }
    }

    pub fn incident(&self) -> &Incident {
        match self {
            IncidentLifecycleEvent::Opened(incident)
            | IncidentLifecycleEvent::Escalated { incident, .. }
            | IncidentLifecycleEvent::Resolved(incident) => incident,
        }
    }
}
