//! In-memory storage impls for tests.

pub mod incident_repository;
pub mod observation_store;

pub use incident_repository::MemoryIncidentRepository;
pub use observation_store::MemoryObservationStore;
