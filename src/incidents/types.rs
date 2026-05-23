use serde::{Deserialize, Serialize};

use chrono::{DateTime, Utc};

use crate::shared::parse::{parse_dotted_name, ParseDottedNameError};
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

/// Canonical name for an incident kind (e.g. `bitcoin.tip_lag`).
///
/// Constructed only through [`IncidentKind::parse`] or
/// [`IncidentKind::from_well_known`]; the inner field is private so
/// callers can't bypass validation by wrapping arbitrary strings (per
/// ADR-D2).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct IncidentKind(String);

impl IncidentKind {
    /// Parse `s` against the shared dotted-name grammar.
    pub fn parse(s: impl AsRef<str>) -> Result<Self, ParseDottedNameError> {
        parse_dotted_name(s.as_ref()).map(Self)
    }

    /// Borrow the inner string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Lift a canonical name from [`crate::incidents::well_known`] into
    /// a typed `IncidentKind`. Rules use this to reference the kind
    /// they emit signals for without re-typing the string literal —
    /// drift between the constant and `default_kinds.toml` is caught
    /// by the parity test in `well_known`.
    ///
    /// Debug-asserts validity against the parse rule; release builds
    /// skip the check because the parity tests in `well_known` and the
    /// `[a-z][a-z0-9_]*` form of the constants make a malformed name
    /// unable to reach `main`.
    pub fn from_well_known(name: &'static str) -> Self {
        debug_assert!(
            parse_dotted_name(name).is_ok(),
            "invalid well_known incident kind: {name}"
        );
        IncidentKind(name.to_string())
    }
}

impl AsRef<str> for IncidentKind {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for IncidentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for IncidentKind {
    type Error = ParseDottedNameError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl From<IncidentKind> for String {
    fn from(k: IncidentKind) -> String {
        k.0
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
    /// Scoped sub-entity subjects (per ADR-N1) render `subject_id` as
    /// `<node_id>/<sub_id>` so cross-node collisions are impossible.
    pub fn as_key(&self) -> String {
        let dim = self.dimension.as_deref().unwrap_or("-");
        format!(
            "{}|{}|{}|{}",
            self.subject.subject_kind_str(),
            self.subject.subject_id_str(),
            self.kind.as_str(),
            dim
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::types::BitcoinNodeId;

    fn fp_btc(kind: &str, dim: Option<&str>) -> IncidentFingerprint {
        IncidentFingerprint {
            subject: EntityRef::BitcoinNode(BitcoinNodeId("alice".into())),
            kind: IncidentKind::parse(kind).expect("valid test kind"),
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

    #[test]
    fn as_key_format_for_sidecar_subject() {
        use crate::shared::types::SidecarId;
        let id = uuid::Uuid::nil();
        let fp = IncidentFingerprint {
            subject: EntityRef::Sidecar(SidecarId(id)),
            kind: IncidentKind::parse("sidecar.collector_failing").expect("valid"),
            dimension: None,
        };
        assert_eq!(
            fp.as_key(),
            format!("sidecar|{id}|sidecar.collector_failing|-")
        );
    }

    #[test]
    fn as_key_format_for_scoped_lnd_channel() {
        use crate::shared::types::{LndChannelId, LndNodeId};
        let fp = IncidentFingerprint {
            subject: EntityRef::LndChannel {
                node_id: LndNodeId("ln-a".into()),
                channel_id: LndChannelId("123:1:0".into()),
            },
            kind: IncidentKind::parse("lnd.channel_inactive").expect("valid"),
            dimension: None,
        };
        assert_eq!(
            fp.as_key(),
            "lnd_channel|ln-a/123:1:0|lnd.channel_inactive|-"
        );
    }

    /// The whole point of ADR-N1 §N1.2: the same channel ID under two
    /// different nodes must produce two different keys.
    #[test]
    fn as_key_distinguishes_same_sub_id_under_different_parents() {
        use crate::shared::types::{LndChannelId, LndNodeId};
        let a = IncidentFingerprint {
            subject: EntityRef::LndChannel {
                node_id: LndNodeId("ln-a".into()),
                channel_id: LndChannelId("123:1:0".into()),
            },
            kind: IncidentKind::parse("lnd.channel_inactive").expect("valid"),
            dimension: None,
        };
        let b = IncidentFingerprint {
            subject: EntityRef::LndChannel {
                node_id: LndNodeId("ln-b".into()),
                channel_id: LndChannelId("123:1:0".into()),
            },
            kind: IncidentKind::parse("lnd.channel_inactive").expect("valid"),
            dimension: None,
        };
        assert_ne!(a.as_key(), b.as_key());
    }

    #[test]
    fn parse_rejects_invalid_input() {
        assert!(IncidentKind::parse("BadCase").is_err());
        assert!(IncidentKind::parse("no_dot").is_err());
        assert!(IncidentKind::parse("").is_err());
    }

    #[test]
    fn parse_accepts_valid_input() {
        let k = IncidentKind::parse("bitcoin.tip_lag").expect("valid");
        assert_eq!(k.as_str(), "bitcoin.tip_lag");
        assert_eq!(format!("{}", k), "bitcoin.tip_lag");
        assert_eq!(<IncidentKind as AsRef<str>>::as_ref(&k), "bitcoin.tip_lag");
    }

    #[test]
    fn from_well_known_constructs() {
        let k = IncidentKind::from_well_known(crate::incidents::well_known::BITCOIN_NO_PEERS);
        assert_eq!(k.as_str(), "bitcoin.no_peers");
    }

    #[test]
    fn serde_round_trips_through_string() {
        let k = IncidentKind::parse("bitcoin.tip_lag").expect("valid");
        let json = serde_json::to_string(&k).expect("serialize");
        assert_eq!(json, "\"bitcoin.tip_lag\"");
        let back: IncidentKind = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, k);
    }

    #[test]
    fn serde_revalidates_invalid_input() {
        // An attacker-controlled JSON value with a malformed name must
        // fail deserialization — the `try_from = "String"` attribute on
        // `IncidentKind` is what makes this work; serde's default
        // `Deserialize` would happily wrap the bad string.
        let err = serde_json::from_str::<IncidentKind>("\"BadCase\"")
            .expect_err("malformed name must not deserialize");
        assert!(err.to_string().contains("invalid character"));
    }

    #[test]
    fn try_from_string_validates() {
        assert!(<IncidentKind as TryFrom<String>>::try_from("bitcoin.tip_lag".into()).is_ok());
        assert!(<IncidentKind as TryFrom<String>>::try_from("BadCase".into()).is_err());
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
