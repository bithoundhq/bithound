//! Canonical incident-kind name constants.
//!
//! Rules and tests construct an [`crate::incidents::IncidentKind`] from one
//! of these constants instead of typing the string literal. The set of
//! constants here is kept in lockstep with `config/default_kinds.toml` by
//! the parity test below — adding a kind to one without the other will
//! fail the test suite.

pub const BITCOIN_TIP_LAG: &str = "bitcoin.tip_lag";
pub const BITCOIN_IBD_STALL: &str = "bitcoin.ibd_stall";
pub const BITCOIN_PEER_STARVATION: &str = "bitcoin.peer_starvation";
pub const BITCOIN_MEMPOOL_FULL: &str = "bitcoin.mempool_full";
pub const BITCOIN_REORG_DEEP: &str = "bitcoin.reorg_deep";
pub const HOST_DISK_EXHAUSTION: &str = "host.disk_exhaustion";
pub const LND_CHANNEL_INACTIVE: &str = "lnd.channel_inactive";
pub const LND_HTLC_STUCK: &str = "lnd.htlc_stuck";
pub const SIDECAR_COLLECTOR_FAILING: &str = "sidecar.collector_failing";

/// All built-in incident-kind names, used by the parity test to compare
/// against `config/default_kinds.toml`.
pub const ALL_BUILTIN_KINDS: &[&str] = &[
    BITCOIN_TIP_LAG,
    BITCOIN_IBD_STALL,
    BITCOIN_PEER_STARVATION,
    BITCOIN_MEMPOOL_FULL,
    BITCOIN_REORG_DEEP,
    HOST_DISK_EXHAUSTION,
    LND_CHANNEL_INACTIVE,
    LND_HTLC_STUCK,
    SIDECAR_COLLECTOR_FAILING,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::incidents::kinds::KindRegistry;
    use crate::incidents::types::IncidentKind;
    use std::collections::BTreeSet;

    #[test]
    fn well_known_constants_match_default_kinds_toml() {
        let registry = KindRegistry::load(None).expect("default kinds load");

        let from_constants: BTreeSet<String> =
            ALL_BUILTIN_KINDS.iter().map(|s| (*s).to_string()).collect();
        assert_eq!(
            from_constants.len(),
            ALL_BUILTIN_KINDS.len(),
            "ALL_BUILTIN_KINDS contains duplicates"
        );

        let from_toml: BTreeSet<String> = registry.iter().map(|spec| spec.name.clone()).collect();

        assert_eq!(
            from_constants, from_toml,
            "well_known.rs and config/default_kinds.toml are out of sync"
        );
    }

    #[test]
    fn every_constant_resolves_in_registry() {
        let registry = KindRegistry::load(None).expect("default kinds load");
        for name in ALL_BUILTIN_KINDS {
            assert!(
                registry
                    .lookup(&IncidentKind((*name).to_string()))
                    .is_some(),
                "well_known constant {name} missing from default_kinds.toml"
            );
        }
    }
}
