//! Canonical incident-kind name constants.
//!
//! Rules and the engine reference kinds through these constants rather
//! than re-typing string literals, so a typo can't quietly desync a
//! rule from the registry. A parity test in this module asserts that
//! every constant in [`ALL`] is also present in the embedded
//! `config/default_kinds.toml` catalog.

pub const BITCOIN_RPC_UNREACHABLE: &str = "bitcoin.rpc_unreachable";
pub const BITCOIN_NO_PEERS: &str = "bitcoin.no_peers";
pub const BITCOIN_TIP_LAG_OR_IBD_STALLED: &str = "bitcoin.tip_lag_or_ibd_stalled";

/// All canonical incident-kind names shipped in the built-in catalog.
///
/// The parity test in this module ensures this slice matches the
/// embedded `config/default_kinds.toml`.
pub const ALL: &[&str] = &[
    BITCOIN_RPC_UNREACHABLE,
    BITCOIN_NO_PEERS,
    BITCOIN_TIP_LAG_OR_IBD_STALLED,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::incidents::kinds::KindRegistry;
    use crate::incidents::types::IncidentKind;
    use std::collections::HashSet;

    #[test]
    fn all_constants_are_unique() {
        let set: HashSet<&&str> = ALL.iter().collect();
        assert_eq!(set.len(), ALL.len(), "well_known::ALL has duplicates");
    }

    /// The embedded default catalog must register exactly the set of
    /// kinds named in [`ALL`]. Drift in either direction is a build
    /// failure so rules can rely on `IncidentKind::from_well_known(...)`
    /// resolving against the registry.
    #[test]
    fn embedded_default_kinds_match_well_known_constants() {
        let registry = KindRegistry::load(None).expect("embedded default kinds load");

        for name in ALL {
            let kind = IncidentKind((*name).into());
            assert!(
                registry.lookup(&kind).is_some(),
                "well_known constant {name:?} missing from default_kinds.toml",
            );
        }
    }
}
