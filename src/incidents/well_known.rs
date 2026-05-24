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
pub const LND_CHANNEL_INACTIVE: &str = "lnd.channel_inactive";
pub const LND_CHAIN_BACKEND_LAG: &str = "lnd.chain_backend_lag";
pub const BITHOUND_LND_UNREACHABLE: &str = "bithound.lnd_unreachable";
pub const BITHOUND_LND_AUTH_FAILED: &str = "bithound.lnd_auth_failed";
pub const BITHOUND_LND_TLS_INVALID: &str = "bithound.lnd_tls_invalid";

/// All canonical incident-kind names shipped in the built-in catalog.
///
/// The parity tests in this module ensure this slice matches the
/// embedded `config/default_kinds.toml` in **both** directions: every
/// constant must be in the registry, and every registry entry must be
/// a constant. Drift in either direction is a build failure.
pub const ALL: &[&str] = &[
    BITCOIN_RPC_UNREACHABLE,
    BITCOIN_NO_PEERS,
    BITCOIN_TIP_LAG_OR_IBD_STALLED,
    LND_CHANNEL_INACTIVE,
    LND_CHAIN_BACKEND_LAG,
    BITHOUND_LND_UNREACHABLE,
    BITHOUND_LND_AUTH_FAILED,
    BITHOUND_LND_TLS_INVALID,
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

    /// Every constant in [`ALL`] must be registered in
    /// `default_kinds.toml`. The reverse-direction check is the
    /// `embedded_default_kinds_subset_of_well_known_constants` test
    /// below.
    #[test]
    fn embedded_default_kinds_match_well_known_constants() {
        let registry = KindRegistry::load(None).expect("embedded default kinds load");

        for name in ALL {
            let kind = IncidentKind::from_well_known(name);
            assert!(
                registry.lookup(&kind).is_some(),
                "well_known constant {name:?} missing from default_kinds.toml",
            );
        }
    }

    /// Reverse direction of the parity check: every kind registered
    /// in `default_kinds.toml` must have a matching constant in
    /// [`ALL`]. Without this, a contributor could add a TOML entry
    /// without updating the constants and rules referencing the
    /// missing constant would fail at runtime via
    /// `IncidentKind::from_well_known` rather than at build time.
    #[test]
    fn embedded_default_kinds_subset_of_well_known_constants() {
        let registry = KindRegistry::load(None).expect("embedded default kinds load");
        let well_known: HashSet<String> = ALL.iter().map(|s| s.to_string()).collect();

        for kind in registry.kinds() {
            let name = kind.as_str();
            assert!(
                well_known.contains(name),
                "default_kinds.toml entry {name:?} missing from well_known::ALL",
            );
        }
    }

    /// Every constant in [`ALL`] must satisfy the shared dotted-name
    /// grammar so `IncidentKind::from_well_known` is a safe fast path
    /// in release builds (where the debug-assert is compiled out).
    #[test]
    fn all_constants_parse_as_dotted_names() {
        use crate::shared::parse::parse_dotted_name;
        for name in ALL {
            parse_dotted_name(name)
                .unwrap_or_else(|e| panic!("well_known constant {name:?} fails parse: {e}"));
        }
    }
}
