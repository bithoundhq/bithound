use crate::{
    incidents::IncidentKind,
    observations::{IncidentSignalObservation, SignalName},
    read_models::Projected,
    shared::types::EntityRef,
};

// This is useful for the incident engine to know current active signals.
pub trait IncidentSignalReadModel: Send + Sync + std::fmt::Debug {
    fn current_signal(
        &self,
        subject: &EntityRef,
        signal: &SignalName,
    ) -> Option<Projected<IncidentSignalObservation>>;

    fn active_signals_for(&self, subject: &EntityRef) -> Vec<Projected<IncidentSignalObservation>>;

    fn active_signals_for_incident_kind(
        &self,
        subject: &EntityRef,
        incident_kind: &IncidentKind,
    ) -> Vec<Projected<IncidentSignalObservation>>;
}
