//! Generic state read-model trait.
//!
//! Per ADR-R1 §R1.1, `StateReadModel` exposes state observations by name
//! rather than per-variant. Sub-variants (e.g. `BitcoinBlockchainState`,
//! `LndNodeState`) are collector-side concerns; the read-model layer is a
//! uniform query surface across shapes. Consumers either pattern-match the
//! returned [`StateObservation`] or use the typed-helper [`StateReadModelExt`]
//! extension trait (auto-implemented for any `StateReadModel`).

use crate::{
    observations::{StateName, StateObservation},
    read_models::Projected,
    shared::types::EntityRef,
};

/// Generic state-observation query surface.
///
/// Implemented by `ReadModelStore` (BTH-25) over the
/// `StateProjection` (BTH-21).
pub trait StateReadModel: Send + Sync + std::fmt::Debug {
    /// Latest state observation of the given name for the given subject.
    ///
    /// Returns `None` if no state with that name has been observed for
    /// the subject. The returned [`Projected`] carries the originating
    /// `observation_id` and `observed_at`; consumers pattern-match
    /// `proj.value` to extract the typed payload.
    fn latest_state(
        &self,
        subject: &EntityRef,
        name: &StateName,
    ) -> Option<Projected<StateObservation>>;

    /// All known state observations for a subject (one per state name).
    ///
    /// Order is unspecified; callers that care about a specific name
    /// should use [`Self::latest_state`].
    fn states_for(&self, subject: &EntityRef) -> Vec<Projected<StateObservation>>;
}
