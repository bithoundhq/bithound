use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::shared::types::*;

mod capability;
mod diagnosis;
mod event;
mod health;
mod incident_signal;
mod inventory;
mod metric;
mod source;
mod state;
mod transition;

pub use capability::*;
pub use event::*;
pub use health::*;
pub use incident_signal::*;
pub use inventory::*;
pub use metric::*;
pub use source::*;
pub use state::*;
pub use transition::*;

#[derive(Debug, Clone)]
pub struct ObservationContext {
    pub source: ObservationSource,
    pub subject: EntityRef,
    pub observed_at: Timestamp,
    pub origin: ObservationOrigin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub id: ObservationId,
    pub observed_at: Timestamp,
    pub received_at: Option<Timestamp>,
    pub source: ObservationSource,
    pub subject: EntityRef,
    pub origin: ObservationOrigin,
    pub attributes: Attributes,
    pub payload: ObservationPayload,
}

impl Observation {
    pub fn metric(
        ctx: ObservationContext,
        name: impl Into<String>,
        kind: MetricKind,
        value: MetricValue,
        unit: Unit,
        attributes: Attributes,
    ) -> Self {
        Self {
            id: ObservationId::new(),
            observed_at: ctx.observed_at,
            received_at: None,
            source: ctx.source,
            subject: ctx.subject,
            origin: ctx.origin,
            attributes,
            payload: ObservationPayload::Metric(MetricObservation {
                name: MetricName(name.into()),
                kind,
                value,
                unit,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationBatch {
    pub batch_id: Uuid,
    pub sidecar_id: SidecarId,
    pub emitted_at: Timestamp,
    pub observations: Vec<Observation>,
}

/// Small, bounded context fields attached to an observation.
/// Equivalent to labels/tags/dimensions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attributes(pub BTreeMap<String, AttributeValue>);

/// Attribute values should not be arbitrary JSON.
/// Thus we keep them bounded through an enum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AttributeValue {
    String(String),
    Bool(bool),
    I64(i64),
    U64(u64),
    F64(f64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObservationPayload {
    Capability(CapabilityObservation),
    Event(EventObservation),
    Heartbeat(HeartbeatObservation),
    Health(HealthCheckObservation),
    Inventory(InventoryObservation),
    Metric(MetricObservation),
    State(StateObservation),
    Transition(TransitionObservation),
}
