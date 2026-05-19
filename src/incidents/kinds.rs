//! Incident-kind registry — built-in + additive user catalog.
//!
//! The registry is loaded once at startup; the engine consults it on every
//! incoming [`IncidentSignalDraft`] to reject malformed drafts before any
//! incident state is mutated.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use thiserror::Error;

use crate::diagnostics::types::IncidentSignalDraft;
use crate::incidents::types::IncidentKind;
use crate::observations::Confidence;
use crate::shared::types::EntitySubjectKind;

/// Built-in defaults embedded at compile time.
const BUILTIN_KINDS_TOML: &str = include_str!("../../config/default_kinds.toml");

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KindSource {
    Builtin,
    UserConfig,
}

#[derive(Debug, Clone)]
pub struct IncidentKindSpec {
    pub name: String,
    pub allowed_subjects: Vec<EntitySubjectKind>,
    pub allows_dimension: bool,
    pub dimension_label: Option<String>,
    pub min_open_confidence: Confidence,
    pub source: KindSource,
}

#[derive(Debug)]
pub struct KindRegistry {
    kinds: HashMap<IncidentKind, IncidentKindSpec>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RegistryError {
    #[error("invalid TOML: {0}")]
    InvalidToml(String),
    #[error("duplicate incident kind: {0:?}")]
    DuplicateKind(IncidentKind),
    #[error("user config cannot override built-in kind: {0:?}")]
    CannotOverrideBuiltin(IncidentKind),
    #[error("unknown subject-kind name: {0}")]
    UnknownSubjectKind(String),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DraftError {
    #[error("unknown incident kind: {0:?}")]
    UnknownKind(IncidentKind),
    #[error("subject {subject:?} not allowed for kind {kind:?}; allowed: {allowed:?}")]
    DisallowedSubject {
        kind: IncidentKind,
        subject: EntitySubjectKind,
        allowed: Vec<EntitySubjectKind>,
    },
    #[error("kind {0:?} requires a dimension")]
    DimensionRequired(IncidentKind),
    #[error("kind {0:?} does not allow a dimension")]
    DimensionForbidden(IncidentKind),
}

impl KindRegistry {
    /// Load the registry from the embedded built-in catalog and, optionally,
    /// a user-supplied TOML file. The user catalog is additive: it cannot
    /// override built-in kinds.
    pub fn load(user_config: Option<&Path>) -> Result<Self, RegistryError> {
        let user_toml = match user_config {
            Some(path) => Some(
                std::fs::read_to_string(path)
                    .map_err(|e| RegistryError::InvalidToml(e.to_string()))?,
            ),
            None => None,
        };
        Self::load_from_toml_strs(BUILTIN_KINDS_TOML, user_toml.as_deref())
    }

    /// Test-facing variant of [`Self::load`] that takes raw TOML for both
    /// catalogs. The engine should use [`Self::load`] in production.
    pub(crate) fn load_from_toml_strs(
        builtin_toml: &str,
        user_toml: Option<&str>,
    ) -> Result<Self, RegistryError> {
        let mut kinds: HashMap<IncidentKind, IncidentKindSpec> = HashMap::new();
        insert_from_toml(&mut kinds, builtin_toml, KindSource::Builtin)?;
        if let Some(user_toml) = user_toml {
            insert_from_toml(&mut kinds, user_toml, KindSource::UserConfig)?;
        }
        Ok(Self { kinds })
    }

    pub fn lookup(&self, kind: &IncidentKind) -> Option<&IncidentKindSpec> {
        self.kinds.get(kind)
    }

    /// Iterate over every registered kind spec. Order is unspecified.
    pub fn iter(&self) -> impl Iterator<Item = &IncidentKindSpec> {
        self.kinds.values()
    }

    pub fn len(&self) -> usize {
        self.kinds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }

    /// Validate a draft against the registry — used by the engine on every
    /// incoming draft. On error, the engine must reject the draft without
    /// mutating any incident state.
    pub fn validate_draft(&self, draft: &IncidentSignalDraft) -> Result<(), DraftError> {
        let spec = self
            .kinds
            .get(&draft.kind)
            .ok_or_else(|| DraftError::UnknownKind(draft.kind.clone()))?;

        let subject_kind = draft.subject.subject_kind();
        if !spec.allowed_subjects.contains(&subject_kind) {
            return Err(DraftError::DisallowedSubject {
                kind: draft.kind.clone(),
                subject: subject_kind,
                allowed: spec.allowed_subjects.clone(),
            });
        }

        match (spec.allows_dimension, draft.dimension.is_some()) {
            (true, false) => Err(DraftError::DimensionRequired(draft.kind.clone())),
            (false, true) => Err(DraftError::DimensionForbidden(draft.kind.clone())),
            _ => Ok(()),
        }
    }
}

fn insert_from_toml(
    kinds: &mut HashMap<IncidentKind, IncidentKindSpec>,
    toml_str: &str,
    source: KindSource,
) -> Result<(), RegistryError> {
    let parsed: KindsToml =
        toml::from_str(toml_str).map_err(|e| RegistryError::InvalidToml(e.to_string()))?;

    for entry in parsed.kinds {
        let spec = parse_entry(entry, source.clone())?;
        let key = IncidentKind(spec.name.clone());

        if let Some(existing) = kinds.get(&key) {
            return Err(match (existing.source.clone(), &source) {
                (KindSource::Builtin, KindSource::UserConfig) => {
                    RegistryError::CannotOverrideBuiltin(key)
                }
                _ => RegistryError::DuplicateKind(key),
            });
        }

        kinds.insert(key, spec);
    }
    Ok(())
}

fn parse_entry(
    entry: KindEntryToml,
    source: KindSource,
) -> Result<IncidentKindSpec, RegistryError> {
    let mut allowed_subjects = Vec::with_capacity(entry.allowed_subjects.len());
    for name in &entry.allowed_subjects {
        allowed_subjects.push(parse_subject_kind(name)?);
    }

    Ok(IncidentKindSpec {
        name: entry.name,
        allowed_subjects,
        allows_dimension: entry.allows_dimension,
        dimension_label: entry.dimension_label,
        min_open_confidence: entry.min_open_confidence.unwrap_or(Confidence::Medium),
        source,
    })
}

fn parse_subject_kind(name: &str) -> Result<EntitySubjectKind, RegistryError> {
    match name {
        "Host" => Ok(EntitySubjectKind::Host),
        "BitcoinNode" => Ok(EntitySubjectKind::BitcoinNode),
        "BitcoinPeer" => Ok(EntitySubjectKind::BitcoinPeer),
        "LndNode" => Ok(EntitySubjectKind::LndNode),
        "LndPeer" => Ok(EntitySubjectKind::LndPeer),
        "LndChannel" => Ok(EntitySubjectKind::LndChannel),
        "LndInvoice" => Ok(EntitySubjectKind::LndInvoice),
        other => Err(RegistryError::UnknownSubjectKind(other.to_string())),
    }
}

#[derive(Debug, Deserialize)]
struct KindsToml {
    #[serde(default)]
    kinds: Vec<KindEntryToml>,
}

#[derive(Debug, Deserialize)]
struct KindEntryToml {
    name: String,
    allowed_subjects: Vec<String>,
    allows_dimension: bool,
    #[serde(default)]
    dimension_label: Option<String>,
    #[serde(default)]
    min_open_confidence: Option<Confidence>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observations::{SignalName, SignalSeverity, SignalStatus};
    use crate::shared::types::{BitcoinNodeId, EntityRef, HostId, LndChannelId, LndNodeId};

    fn draft(subject: EntityRef, kind: &str, dimension: Option<&str>) -> IncidentSignalDraft {
        IncidentSignalDraft {
            subject,
            signal: SignalName(format!("{kind}.signal")),
            kind: IncidentKind(kind.into()),
            dimension: dimension.map(str::to_string),
            severity: SignalSeverity::Warning,
            status: SignalStatus::Active,
            confidence: Confidence::High,
            evidence: vec![],
        }
    }

    const SAMPLE_BUILTIN: &str = r#"
[[kinds]]
name = "bitcoin.tip_lag"
allowed_subjects = ["BitcoinNode"]
allows_dimension = false

[[kinds]]
name = "host.disk_exhaustion"
allowed_subjects = ["Host"]
allows_dimension = true
dimension_label = "mount_path"
min_open_confidence = "High"
"#;

    fn registry() -> KindRegistry {
        KindRegistry::load_from_toml_strs(SAMPLE_BUILTIN, None).expect("load")
    }

    #[test]
    fn load_parses_builtin_fields() {
        let reg = registry();

        let tip_lag = reg
            .lookup(&IncidentKind("bitcoin.tip_lag".into()))
            .expect("known kind");
        assert_eq!(
            tip_lag.allowed_subjects,
            vec![EntitySubjectKind::BitcoinNode]
        );
        assert!(!tip_lag.allows_dimension);
        assert_eq!(tip_lag.dimension_label, None);
        assert_eq!(tip_lag.min_open_confidence, Confidence::Medium);
        assert_eq!(tip_lag.source, KindSource::Builtin);

        let disk = reg
            .lookup(&IncidentKind("host.disk_exhaustion".into()))
            .expect("known kind");
        assert!(disk.allows_dimension);
        assert_eq!(disk.dimension_label.as_deref(), Some("mount_path"));
        assert_eq!(disk.min_open_confidence, Confidence::High);
    }

    #[test]
    fn validate_draft_happy_path() {
        let reg = registry();
        let ok = draft(
            EntityRef::BitcoinNode(BitcoinNodeId("alice".into())),
            "bitcoin.tip_lag",
            None,
        );
        assert_eq!(reg.validate_draft(&ok), Ok(()));

        let ok_dim = draft(
            EntityRef::Host(HostId("h1".into())),
            "host.disk_exhaustion",
            Some("/var/lib/bitcoin"),
        );
        assert_eq!(reg.validate_draft(&ok_dim), Ok(()));
    }

    #[test]
    fn validate_draft_unknown_kind() {
        let reg = registry();
        let bad = draft(
            EntityRef::BitcoinNode(BitcoinNodeId("alice".into())),
            "bitcoin.nonexistent",
            None,
        );
        assert_eq!(
            reg.validate_draft(&bad),
            Err(DraftError::UnknownKind(IncidentKind(
                "bitcoin.nonexistent".into()
            )))
        );
    }

    #[test]
    fn validate_draft_disallowed_subject() {
        let reg = registry();
        let bad = draft(
            EntityRef::LndNode(LndNodeId("ln1".into())),
            "bitcoin.tip_lag",
            None,
        );
        assert_eq!(
            reg.validate_draft(&bad),
            Err(DraftError::DisallowedSubject {
                kind: IncidentKind("bitcoin.tip_lag".into()),
                subject: EntitySubjectKind::LndNode,
                allowed: vec![EntitySubjectKind::BitcoinNode],
            })
        );
    }

    #[test]
    fn validate_draft_dimension_required() {
        let reg = registry();
        let bad = draft(
            EntityRef::Host(HostId("h1".into())),
            "host.disk_exhaustion",
            None,
        );
        assert_eq!(
            reg.validate_draft(&bad),
            Err(DraftError::DimensionRequired(IncidentKind(
                "host.disk_exhaustion".into()
            )))
        );
    }

    #[test]
    fn validate_draft_dimension_forbidden() {
        let reg = registry();
        let bad = draft(
            EntityRef::BitcoinNode(BitcoinNodeId("alice".into())),
            "bitcoin.tip_lag",
            Some("extra"),
        );
        assert_eq!(
            reg.validate_draft(&bad),
            Err(DraftError::DimensionForbidden(IncidentKind(
                "bitcoin.tip_lag".into()
            )))
        );
    }

    #[test]
    fn user_config_cannot_override_builtin() {
        let user = r#"
[[kinds]]
name = "bitcoin.tip_lag"
allowed_subjects = ["BitcoinNode"]
allows_dimension = false
"#;
        let err = KindRegistry::load_from_toml_strs(SAMPLE_BUILTIN, Some(user)).unwrap_err();
        assert_eq!(
            err,
            RegistryError::CannotOverrideBuiltin(IncidentKind("bitcoin.tip_lag".into()))
        );
    }

    #[test]
    fn user_config_adds_new_kind() {
        let user = r#"
[[kinds]]
name = "operator.custom_check"
allowed_subjects = ["LndChannel"]
allows_dimension = false
"#;
        let reg = KindRegistry::load_from_toml_strs(SAMPLE_BUILTIN, Some(user)).expect("load");
        let spec = reg
            .lookup(&IncidentKind("operator.custom_check".into()))
            .expect("user kind");
        assert_eq!(spec.source, KindSource::UserConfig);

        let ok = draft(
            EntityRef::LndChannel(LndChannelId("chan1".into())),
            "operator.custom_check",
            None,
        );
        assert_eq!(reg.validate_draft(&ok), Ok(()));
    }

    #[test]
    fn duplicate_kind_within_one_catalog_rejected() {
        let builtin = r#"
[[kinds]]
name = "bitcoin.tip_lag"
allowed_subjects = ["BitcoinNode"]
allows_dimension = false

[[kinds]]
name = "bitcoin.tip_lag"
allowed_subjects = ["BitcoinNode"]
allows_dimension = false
"#;
        let err = KindRegistry::load_from_toml_strs(builtin, None).unwrap_err();
        assert_eq!(
            err,
            RegistryError::DuplicateKind(IncidentKind("bitcoin.tip_lag".into()))
        );
    }

    #[test]
    fn unknown_subject_kind_rejected() {
        let builtin = r#"
[[kinds]]
name = "bitcoin.tip_lag"
allowed_subjects = ["Spaceship"]
allows_dimension = false
"#;
        let err = KindRegistry::load_from_toml_strs(builtin, None).unwrap_err();
        assert_eq!(err, RegistryError::UnknownSubjectKind("Spaceship".into()));
    }

    #[test]
    fn invalid_toml_rejected() {
        let err = KindRegistry::load_from_toml_strs("not = valid = toml", None).unwrap_err();
        assert!(matches!(err, RegistryError::InvalidToml(_)));
    }

    #[test]
    fn embedded_builtin_catalog_loads() {
        // BTH-15 ships an empty `config/default_kinds.toml`; BTH-16 populates
        // it. This test guards against a malformed default catalog.
        KindRegistry::load(None).expect("embedded default catalog loads");
    }
}
