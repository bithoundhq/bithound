use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::collectors::{CollectionError, CollectorRef};
use crate::shared::types::*;

mod capability;
mod diagnosis;
mod event;
mod health;
mod incident_signal;
mod inventory;
mod metric;
mod source;
pub mod state;
mod transition;

pub use capability::*;
pub use diagnosis::*;
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
    pub observed_at: DateTime<Utc>,
    pub origin: ObservationOrigin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub id: ObservationId,
    pub observed_at: DateTime<Utc>,
    pub received_at: Option<DateTime<Utc>>,
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

    #[allow(clippy::too_many_arguments)]
    pub fn heartbeat(
        ctx: ObservationContext,
        sequence: u64,
        sidecar_time: DateTime<Utc>,
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
                target: HealthTargetId(target.into()),
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

    pub fn state(ctx: ObservationContext, state: StateObservation, attributes: Attributes) -> Self {
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

    /// Wrap an `IncidentSignalObservation` into a full `Observation`.
    ///
    /// Per ADR-R2, signal observations are first-class observations produced
    /// by the incident engine with `ObservationOrigin::Computed`; the engine
    /// stamps the appropriate source/subject in `ctx`.
    pub fn incident_signal(
        ctx: ObservationContext,
        signal: IncidentSignalObservation,
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
            payload: ObservationPayload::IncidentSignal(signal),
        }
    }

    /// Wrap a `DiagnosisObservation` into a full `Observation`.
    ///
    /// Per ADR-R2, diagnosis observations are first-class observations.
    /// No diagnosis emitter exists in V0; the variant is defined for
    /// forward compatibility so future richer findings can land without
    /// enum churn.
    pub fn diagnosis(
        ctx: ObservationContext,
        diagnosis: DiagnosisObservation,
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
            payload: ObservationPayload::Diagnosis(diagnosis),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationBatch {
    pub id: ObservationBatchId,
    pub collector: CollectorRef,
    pub sidecar_id: SidecarId,
    pub window: ProbeWindow,
    pub result: ProbeResult,
}

/// Time window of a single probe execution. Constructor enforces start ≤ end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeWindow {
    started_at: DateTime<Utc>,
    completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeWindowError {
    Inverted,
}

impl ProbeWindow {
    pub fn new(
        started_at: DateTime<Utc>,
        completed_at: DateTime<Utc>,
    ) -> Result<Self, ProbeWindowError> {
        if completed_at < started_at {
            return Err(ProbeWindowError::Inverted);
        }
        Ok(Self {
            started_at,
            completed_at,
        })
    }

    pub fn started_at(&self) -> DateTime<Utc> {
        self.started_at
    }

    pub fn completed_at(&self) -> DateTime<Utc> {
        self.completed_at
    }

    pub fn duration(&self) -> chrono::Duration {
        self.completed_at - self.started_at
    }
}

/// Outcome of a probe pass. Failed probes always carry a HealthCheckObservation;
/// successful probes never do (health belongs in the observations list if applicable).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProbeResult {
    Ok {
        observations: Vec<Observation>,
    },
    Failed {
        health: HealthCheckObservation,
        partial_observations: Vec<Observation>,
        error: CollectionError,
    },
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
    Diagnosis(DiagnosisObservation),
    Event(EventObservation),
    Heartbeat(HeartbeatObservation),
    Health(HealthCheckObservation),
    IncidentSignal(IncidentSignalObservation),
    Inventory(InventoryObservation),
    Metric(MetricObservation),
    State(StateObservation),
    Transition(TransitionObservation),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collectors::{CollectorRef, IntegrationKind};
    use chrono::Duration;

    fn ctx() -> ObservationContext {
        ObservationContext {
            source: ObservationSource {
                sidecar_id: SidecarId(uuid::Uuid::now_v7()),
                collector: CollectorRef {
                    id: CollectorId("test-collector".into()),
                    integration: IntegrationKind::BitcoinCoreRpc {
                        interval: Duration::seconds(10),
                    },
                    instance_label: "test".into(),
                },
            },
            subject: EntityRef::BitcoinNode(BitcoinNodeId("alice".into())),
            observed_at: Utc::now(),
            origin: ObservationOrigin::Computed,
        }
    }

    #[test]
    fn incident_signal_payload_roundtrips_via_serde() {
        let kind =
            crate::incidents::IncidentKind::parse("bitcoin.no_peers").expect("valid test kind");
        let signal = IncidentSignalObservation {
            signal: SignalName::for_incident_kind(&kind),
            incident_kind: kind,
            severity: SignalSeverity::Critical,
            status: SignalStatus::Active,
            confidence: Confidence::High,
            evidence: vec![],
        };
        let obs = Observation::incident_signal(ctx(), signal, Attributes(BTreeMap::new()));
        let json = serde_json::to_string(&obs).expect("serialize");
        let back: Observation = serde_json::from_str(&json).expect("deserialize");
        assert!(matches!(
            back.payload,
            ObservationPayload::IncidentSignal(_)
        ));
    }

    #[test]
    fn diagnosis_payload_roundtrips_via_serde() {
        let diagnosis = DiagnosisObservation {
            diagnosis: DiagnosisName("bitcoin.tip_lag.assessment".into()),
            summary: "node likely stuck in IBD".into(),
            confidence: Confidence::Medium,
            likely_causes: vec!["maxtipage heuristic".into()],
            recommended_actions: vec!["restart with -maxtipage=...".into()],
            evidence: vec![],
        };
        let obs = Observation::diagnosis(ctx(), diagnosis, Attributes(BTreeMap::new()));
        let json = serde_json::to_string(&obs).expect("serialize");
        let back: Observation = serde_json::from_str(&json).expect("deserialize");
        assert!(matches!(back.payload, ObservationPayload::Diagnosis(_)));
    }
}
