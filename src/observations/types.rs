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

    pub fn capability(
        ctx: ObservationContext,
        name: impl Into<String>,
        status: CapabilityStatus,
        reason: Option<String>,
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
            payload: ObservationPayload::Capability(CapabilityObservation {
                capability: CapabilityName(name.into()),
                status,
                reason,
            }),
        }
    }

    pub fn event(
        ctx: ObservationContext,
        name: impl Into<String>,
        severity: EventSeverity,
        body: Option<String>,
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
            payload: ObservationPayload::Event(EventObservation {
                name: EventName(name.into()),
                severity,
                body,
            }),
        }
    }

    pub fn heartbeat(
        ctx: ObservationContext,
        sequence: u64,
        sidecar_time: Timestamp,
        monotonic_uptime_ms: Option<u64>,
        sidecar_version: impl Into<String>,
        status: HeartbeatStatus,
        collector_statuses: Vec<CollectorStatus>,
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
            payload: ObservationPayload::Heartbeat(HeartbeatObservation {
                sequence,
                sidecar_time,
                monotonic_uptime_ms,
                sidecar_version: sidecar_version.into(),
                status,
                collector_statuses,
            }),
        }
    }

    pub fn health(
        ctx: ObservationContext,
        target: impl Into<String>,
        status: HealthStatus,
        latency_ms: Option<u64>,
        message: Option<String>,
        error: Option<HealthError>,
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
            payload: ObservationPayload::Health(HealthCheckObservation {
                target: HealthTarget(target.into()),
                status,
                latency_ms,
                message,
                error,
            }),
        }
    }

    pub fn inventory(
        ctx: ObservationContext,
        name: impl Into<String>,
        facts: BTreeMap<String, InventoryValue>,
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
            payload: ObservationPayload::Inventory(InventoryObservation {
                name: InventoryName(name.into()),
                facts,
            }),
        }
    }

    pub fn state(
        ctx: ObservationContext,
        state: StateObservation,
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
            payload: ObservationPayload::State(state),
        }
    }

    pub fn transition(
        ctx: ObservationContext,
        name: impl Into<String>,
        from: StateAtom,
        to: StateAtom,
        reason: Option<String>,
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
            payload: ObservationPayload::Transition(TransitionObservation {
                name: TransitionName(name.into()),
                from,
                to,
                reason,
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
