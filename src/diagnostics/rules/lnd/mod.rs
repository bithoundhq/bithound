//! Diagnostic rules for LND nodes and their channels.
//!
//! Re-exports land when the runtime wires the rules into the
//! engine (BTH-68); until then, the rule types are reachable via
//! their full module paths.

pub mod chain_backend_lag;
pub mod channel_inactive;
