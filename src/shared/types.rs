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
    Host(HostId),
    BitcoinNode(BitcoinNodeId),
    BitcoinPeer(BitcoinPeerId),
    LndNode(LndNodeId),
    LndPeer(LndPeerId),
    LndChannel(LndChannelId),
    LndInvoice(LndInvoiceId),
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
            EntityRef::Host(_) => EntitySubjectKind::Host,
            EntityRef::BitcoinNode(_) => EntitySubjectKind::BitcoinNode,
            EntityRef::BitcoinPeer(_) => EntitySubjectKind::BitcoinPeer,
            EntityRef::LndNode(_) => EntitySubjectKind::LndNode,
            EntityRef::LndPeer(_) => EntitySubjectKind::LndPeer,
            EntityRef::LndChannel(_) => EntitySubjectKind::LndChannel,
            EntityRef::LndInvoice(_) => EntitySubjectKind::LndInvoice,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_ref_subject_kind_matches_variant() {
        let pairs: [(EntityRef, EntitySubjectKind); 7] = [
            (EntityRef::Host(HostId("h".into())), EntitySubjectKind::Host),
            (
                EntityRef::BitcoinNode(BitcoinNodeId("n".into())),
                EntitySubjectKind::BitcoinNode,
            ),
            (
                EntityRef::BitcoinPeer(BitcoinPeerId("p".into())),
                EntitySubjectKind::BitcoinPeer,
            ),
            (
                EntityRef::LndNode(LndNodeId("ln".into())),
                EntitySubjectKind::LndNode,
            ),
            (
                EntityRef::LndPeer(LndPeerId("lp".into())),
                EntitySubjectKind::LndPeer,
            ),
            (
                EntityRef::LndChannel(LndChannelId("lc".into())),
                EntitySubjectKind::LndChannel,
            ),
            (
                EntityRef::LndInvoice(LndInvoiceId("li".into())),
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
}
