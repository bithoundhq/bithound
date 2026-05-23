use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CollectorId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IncidentId(pub Uuid);

impl IncidentId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObservationId(pub Uuid);

impl ObservationId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObservationBatchId(pub Uuid);

impl ObservationBatchId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActorId(pub String);

impl ActorId {
    pub fn system() -> Self {
        Self("system".into())
    }

    pub fn operator(name: impl Into<String>) -> Self {
        Self(name.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SidecarId(pub Uuid);

/// Evidence should reference observations.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EvidenceRef(pub ObservationId);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HostId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BitcoinNodeId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BitcoinPeerId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LndNodeId(pub String); // derived from pubkey

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LndPeerId(pub String); // remote pubkey

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LndChannelId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LndInvoiceId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityRef {
    /// The sidecar itself. Used by heartbeats, collector-failure
    /// observations, storage-error observations, and any future
    /// `sidecar.*` incident kinds (ADR-N1 §N1.1).
    Sidecar(SidecarId),
    Host(HostId),
    BitcoinNode(BitcoinNodeId),
    /// Bitcoin peers are protocol-level IDs that are not globally
    /// unique across multiple monitored nodes — the variant is scoped
    /// under its `node_id` so two distinct nodes' peers can never
    /// collapse into one fingerprint (ADR-N1 §N1.2).
    BitcoinPeer {
        node_id: BitcoinNodeId,
        peer_id: BitcoinPeerId,
    },
    LndNode(LndNodeId),
    /// LND peers are scoped under their parent LND node — see
    /// `BitcoinPeer` for the same rationale.
    LndPeer {
        node_id: LndNodeId,
        peer_id: LndPeerId,
    },
    /// LND channel IDs (`scid`-style) are not globally unique;
    /// scoping under `node_id` prevents cross-node collisions.
    LndChannel {
        node_id: LndNodeId,
        channel_id: LndChannelId,
    },
    /// LND invoice IDs (payment hashes) are likewise scoped under
    /// their parent node.
    LndInvoice {
        node_id: LndNodeId,
        invoice_id: LndInvoiceId,
    },
}

/// Named discriminant for [`EntityRef`].
///
/// Used by [`crate::incidents::kinds::IncidentKindSpec::allowed_subjects`]
/// to declare which subject kinds a given incident kind permits.
/// The named-enum form is preferred over `std::mem::discriminant`
/// because it is greppable, serializable, and the exhaustive `match` in
/// [`EntityRef::subject_kind`] turns into a compile error if a new
/// [`EntityRef`] variant is added without updating both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntitySubjectKind {
    Sidecar,
    Host,
    BitcoinNode,
    BitcoinPeer,
    LndNode,
    LndPeer,
    LndChannel,
    LndInvoice,
}

impl EntityRef {
    /// Returns the [`EntitySubjectKind`] corresponding to this reference's variant.
    pub fn subject_kind(&self) -> EntitySubjectKind {
        match self {
            EntityRef::Sidecar(_) => EntitySubjectKind::Sidecar,
            EntityRef::Host(_) => EntitySubjectKind::Host,
            EntityRef::BitcoinNode(_) => EntitySubjectKind::BitcoinNode,
            EntityRef::BitcoinPeer { .. } => EntitySubjectKind::BitcoinPeer,
            EntityRef::LndNode(_) => EntitySubjectKind::LndNode,
            EntityRef::LndPeer { .. } => EntitySubjectKind::LndPeer,
            EntityRef::LndChannel { .. } => EntitySubjectKind::LndChannel,
            EntityRef::LndInvoice { .. } => EntitySubjectKind::LndInvoice,
        }
    }

    /// Stable string tag for the subject's kind. Used as the
    /// `subject_kind` column in the SQLite schema (ADR-P1) and as the
    /// first segment of [`crate::incidents::IncidentFingerprint::as_key`].
    pub fn subject_kind_str(&self) -> &'static str {
        match self {
            EntityRef::Sidecar(_) => "sidecar",
            EntityRef::Host(_) => "host",
            EntityRef::BitcoinNode(_) => "bitcoin_node",
            EntityRef::BitcoinPeer { .. } => "bitcoin_peer",
            EntityRef::LndNode(_) => "lnd_node",
            EntityRef::LndPeer { .. } => "lnd_peer",
            EntityRef::LndChannel { .. } => "lnd_channel",
            EntityRef::LndInvoice { .. } => "lnd_invoice",
        }
    }

    /// Stable string form for the subject's ID. Scoped sub-entity
    /// variants render as `<node_id>/<sub_id>` so cross-node collisions
    /// are impossible (ADR-N1 §N1.3).
    pub fn subject_id_str(&self) -> String {
        match self {
            EntityRef::Sidecar(id) => id.0.to_string(),
            EntityRef::Host(id) => id.0.clone(),
            EntityRef::BitcoinNode(id) => id.0.clone(),
            EntityRef::BitcoinPeer { node_id, peer_id } => format!("{}/{}", node_id.0, peer_id.0),
            EntityRef::LndNode(id) => id.0.clone(),
            EntityRef::LndPeer { node_id, peer_id } => format!("{}/{}", node_id.0, peer_id.0),
            EntityRef::LndChannel {
                node_id,
                channel_id,
            } => format!("{}/{}", node_id.0, channel_id.0),
            EntityRef::LndInvoice {
                node_id,
                invoice_id,
            } => format!("{}/{}", node_id.0, invoice_id.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_ref_subject_kind_matches_variant() {
        let pairs: [(EntityRef, EntitySubjectKind); 8] = [
            (
                EntityRef::Sidecar(SidecarId(Uuid::nil())),
                EntitySubjectKind::Sidecar,
            ),
            (EntityRef::Host(HostId("h".into())), EntitySubjectKind::Host),
            (
                EntityRef::BitcoinNode(BitcoinNodeId("n".into())),
                EntitySubjectKind::BitcoinNode,
            ),
            (
                EntityRef::BitcoinPeer {
                    node_id: BitcoinNodeId("n".into()),
                    peer_id: BitcoinPeerId("p".into()),
                },
                EntitySubjectKind::BitcoinPeer,
            ),
            (
                EntityRef::LndNode(LndNodeId("ln".into())),
                EntitySubjectKind::LndNode,
            ),
            (
                EntityRef::LndPeer {
                    node_id: LndNodeId("ln".into()),
                    peer_id: LndPeerId("lp".into()),
                },
                EntitySubjectKind::LndPeer,
            ),
            (
                EntityRef::LndChannel {
                    node_id: LndNodeId("ln".into()),
                    channel_id: LndChannelId("lc".into()),
                },
                EntitySubjectKind::LndChannel,
            ),
            (
                EntityRef::LndInvoice {
                    node_id: LndNodeId("ln".into()),
                    invoice_id: LndInvoiceId("li".into()),
                },
                EntitySubjectKind::LndInvoice,
            ),
        ];
        for (entity, expected) in pairs {
            assert_eq!(entity.subject_kind(), expected);
        }
    }

    #[test]
    fn entity_subject_kind_serde_roundtrip() {
        let kinds = [
            EntitySubjectKind::Sidecar,
            EntitySubjectKind::Host,
            EntitySubjectKind::BitcoinNode,
            EntitySubjectKind::BitcoinPeer,
            EntitySubjectKind::LndNode,
            EntitySubjectKind::LndPeer,
            EntitySubjectKind::LndChannel,
            EntitySubjectKind::LndInvoice,
        ];
        for kind in kinds {
            let json = serde_json::to_string(&kind).expect("serialize");
            let back: EntitySubjectKind = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(kind, back);
        }
    }

    /// Two `LndChannel` references with the same `channel_id` under
    /// different parent nodes must compare unequal — that's the whole
    /// point of ADR-N1 §N1.2.
    #[test]
    fn scoped_sub_entity_with_same_id_under_different_parents_is_unequal() {
        let a = EntityRef::LndChannel {
            node_id: LndNodeId("ln-a".into()),
            channel_id: LndChannelId("123:1:0".into()),
        };
        let b = EntityRef::LndChannel {
            node_id: LndNodeId("ln-b".into()),
            channel_id: LndChannelId("123:1:0".into()),
        };
        assert_ne!(a, b);
    }

    #[test]
    fn sidecar_entity_ref_serde_roundtrip() {
        let r = EntityRef::Sidecar(SidecarId(Uuid::now_v7()));
        let json = serde_json::to_string(&r).expect("serialize");
        let back: EntityRef = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(r, back);
        assert_eq!(back.subject_kind(), EntitySubjectKind::Sidecar);
    }

    #[test]
    fn scoped_lnd_channel_entity_ref_serde_roundtrip() {
        let r = EntityRef::LndChannel {
            node_id: LndNodeId("ln-a".into()),
            channel_id: LndChannelId("123:1:0".into()),
        };
        let json = serde_json::to_string(&r).expect("serialize");
        let back: EntityRef = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(r, back);
    }
}
