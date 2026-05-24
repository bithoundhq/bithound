//! Concrete `DiagnosticRule` implementations.
//!
//! Each integration domain owns a submodule (`bitcoin`, eventually
//! `lnd`, `host`, …). Rules are intentionally small and self-contained:
//! they read from the typed read-model surface in
//! [`crate::diagnostics::types::DiagnosticContext`], own their own
//! hysteresis state, and emit `IncidentSignalDraft`s through the
//! [`crate::diagnostics::traits::DiagnosticRule`] trait.

pub mod bitcoin;
pub mod lnd;
