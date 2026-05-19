# Bithound Domain Specification

> **Companion documents.**
> - `IMPLEMENTATION_PLAN.md` — phases, milestones, dependency graph,
>   parallelization-friendly subsets, and per-ticket estimates.
> - `TICKETS.md` — 41 JIRA-style tickets (BTH-1 … BTH-41) implementing
>   every ADR in this spec. Each ticket lists its ADR references,
>   acceptance criteria, and blocking dependencies.
>
> Use `SPEC.md` to understand **what** Bithound is and **why** every
> decision was made; use `IMPLEMENTATION_PLAN.md` and `TICKETS.md` to
> understand **how** to build it.

> **Reconciliation status (2026-05-17).** This document was rewritten by reading
> the source tree at `src/`. Where the code disagreed with the prior spec, the
> code wins and the divergence is annotated inline as
> **`Divergence from prior spec:`**. Where a section was still aspirational and
> the code is silent, it's marked **`Not implemented yet`**.
>
> A few high-level facts to set expectations:
>
> - `src/main.rs` is a `println!("hello world")` stub. There is **no runtime,
>   no scheduler, no storage, no API, and no wiring** between modules yet.
>   The codebase is a typed domain model awaiting a runtime.
> - The domain decomposition is real: `targets` is folded into shared identity
>   types; `observations`, `read_models`, `diagnostics`, `incidents`, and
>   `notifications` each have their own module with types and (in most cases)
>   traits.
> - The observation model is far richer than the prior spec described — eight
>   payload variants, strongly typed state, attribute enums, probe windows,
>   batches, and explicit provenance.
> - Incident **fingerprinting and deduplication are absent**. Incidents have
>   identity (`IncidentId`) and lifecycle events but no fingerprint field and
>   no service that records findings.
> - Notification delivery is well-typed (rich `DeliveryOutcome` taxonomy) but
>   the senders are stubs that return `PermanentError::BadRequest("not yet
>   implemented")`. The orchestrator (`Notifier::dispatch`) exists and matches
>   rules against incident events.

---

# 1. System Summary

Bithound is a local-first telemetry agent ("sidecar") for Bitcoin infrastructure.
It observes Bitcoin Core, LND, Elements, and the host system, projects current
state, evaluates diagnostic rules into incident signals, lifts signals into
incidents, and delivers lifecycle changes to notification targets.

The intended runtime flow:

```text
Collectors → Observations → Read Models → Diagnostics (signals)
                                                 ↓
                                         Incident Engine
                                                 ↓
                                    IncidentLifecycleEvent
                                                 ↓
                                          Notifier → sinks
```

> **Divergence from prior spec:** The prior spec drew a single arrow from
> diagnostics straight to incidents. The code introduces an intermediate
> *observation-tier* type — `IncidentSignalObservation` — and a complementary
> `IncidentSignalReadModel`. Diagnostics produce `IncidentSignalDraft`s; those
> are turned into signal observations; an (unbuilt) incident engine consumes
> the signal read model to open/escalate/resolve incidents. This is a
> finer-grained pipeline than the prior spec drew.

---

# 2. Architectural Principles

## 2.1 Collectors do not detect incidents

Confirmed by the code shape: collectors produce `ObservationBatch` values
containing `Observation`s. There is no path from a collector to an incident.

## 2.2 Observations are immutable facts

Confirmed by type design. `Observation` is a `Clone + Serialize + Deserialize`
record with `ObservationId` (UUIDv7), `observed_at`, optional `received_at`,
provenance, subject, origin, attributes, and payload. No mutation API exists.

## 2.3 Read models are projections, not authorities

Confirmed by the `read_models` trait set. Each read model returns
`Projected<T>` which pairs the value with the originating `ObservationId` and
`observed_at` timestamp — i.e. every read-model value carries its evidence
pointer. No mutation methods.

## 2.4 Diagnostics produce findings, not incidents

Confirmed. `DiagnosticRule::evaluate` returns `Vec<IncidentSignalDraft>`.

> **Divergence from prior spec:** The output type is `IncidentSignalDraft`,
> not `DiagnosticFinding`. The intent is the same — diagnostics are advisory
> — but the lexicon is "signal," not "finding," and the draft is destined
> to become an `IncidentSignalObservation` (observation-tier), not an
> incident-tier `Evidence` record.

## 2.5 Incidents own lifecycle and deduplication

Partially. `Incident`, `IncidentStatus`, `IncidentSeverity`, and
`IncidentLifecycleEvent` exist. There is **no fingerprint type**, **no
incident service**, **no repository**, and **no dedup logic** anywhere
in the tree.

## 2.6 Notifications react to incident lifecycle events

Confirmed by `Notifier::dispatch(&IncidentLifecycleEvent, &NotificationMessage)`
in `src/notifications/orchestrator.rs:35-49`. Sinks never see raw observations.

---

# 3. Bounded Contexts

Bithound is a single binary crate (`bithound` 0.1.0) with the following
top-level modules under `src/`:

| Domain | Module | Exists? | Notes |
|---|---|---|---|
| Targets | (folded into `shared::types`) | Partial | No `TargetId` / `MonitoredTarget`. Identity is per-entity via `BitcoinNodeId`, `LndNodeId`, `HostId`, etc. The unified abstraction is `EntityRef` (an enum), not a typed-target wrapper. `SidecarId` exists in `shared` but is not used as a monitoring target. |
| Observations | `src/observations/` | Yes | Rich. Eight payload variants split across submodules. |
| Collectors | `src/collectors/` | Partial | Types & registry present; **`Collector` trait file is empty**. |
| Read Models | `src/read_models/` | Yes | Seven traits, one shared wrapper type. No concrete implementations. |
| Diagnostics | `src/diagnostics/` | Partial | One trait (`DiagnosticRule`), one context, one draft type. No rules implemented. Module is `mod traits; mod types;` — neither is `pub`. |
| Incidents | `src/incidents/` | Partial | `Incident`, lifecycle events, severity, status. No fingerprint, no service. |
| Notifications | `src/notifications/` | Yes (most developed) | Types, orchestrator, three target adapters (Discord, Telegram, Webhook), pairing primitives for Telegram. Senders are stubs. |
| Runtime | none | No | `src/main.rs` is `println!("hello world")`. |
| Storage | none | No | No DB, no migrations, no persistence trait. |
| API | none | No | No HTTP layer. |
| Shared identity | `src/shared/types.rs` | Yes | ID newtypes, `EntityRef`, `EvidenceRef`. |
| RPC | `src/rpc.rs` | No (file is whitespace) | Module is declared but empty. |

---

# 4. Domain Interaction Model

Observed dependency direction (verified by `use` statements):

```text
shared::types ←── observations
shared::types ←── collectors
shared::types ←── read_models ←── observations
shared::types ←── diagnostics  ←── observations, read_models, incidents
shared::types ←── incidents
shared::types ←── notifications ←── incidents
collectors    ←── observations (CollectorRef appears on ObservationSource / ObservationBatch)
```

Notable boundary observations:

- `diagnostics::traits` imports `IncidentSignalDraft` only — no incident
  lifecycle types. Good.
- `read_models::traits::incident_signal` imports `IncidentKind` from
  `incidents` — a read-model trait reaches *up* into the incident domain
  for one type. Acceptable, since `IncidentKind` is currently a plain
  `String` newtype.
- `notifications::types` imports `IncidentLifecycleEvent`, `IncidentKind`,
  `IncidentSeverity`. Notifications never reach into observations or
  collectors.
- No module imports `runtime` because there is no runtime.

No boundary violations were found in the code as written, because no code
exists at the orchestration seams yet to violate them. The model is
constraint-shaped, not behavior-shaped.

---

# 5. Targets / Subjects Domain

> **Divergence from prior spec:** The prior spec called for a unified
> `TargetId(Uuid)` + `TargetKind` enum + `MonitoredTarget` aggregate. The
> code does not do this. Instead, monitored entities are identified by
> domain-specific newtype IDs and unified at the *reference* level via the
> `EntityRef` enum. This treats Bitcoin nodes, peers, channels, invoices,
> and hosts as first-class entities, not as a flat list of "targets."

## 5.1 Identity types

From `src/shared/types.rs:53-71`:

```rust
pub struct HostId(pub String);
pub struct BitcoinNodeId(pub String);
pub struct BitcoinPeerId(pub String);
pub struct LndNodeId(pub String);   // derived from pubkey
pub struct LndPeerId(pub String);   // remote pubkey
pub struct LndChannelId(pub String);
pub struct LndInvoiceId(pub String);
pub struct SidecarId(pub Uuid);
```

All entity IDs are `String` newtypes (typically derived from natural
identifiers like pubkey or hostname). `SidecarId` is a UUID because the
sidecar's identity is not derived externally.

## 5.2 Unified reference

```rust
pub enum EntityRef {
    Host(HostId),
    BitcoinNode(BitcoinNodeId),
    BitcoinPeer(BitcoinPeerId),
    LndNode(LndNodeId),
    LndPeer(LndPeerId),
    LndChannel(LndChannelId),
    LndInvoice(LndInvoiceId),
}
```

`EntityRef` is what observations, read models, and incidents use as
"subject." Every observation has `subject: EntityRef`.

## 5.3 Collector targets

Collectors use a narrower target set (`src/collectors/types.rs:90-95`):

```rust
pub enum CollectorTarget {
    BitcoinNode(BitcoinNodeId),
    LndNode(LndNodeId),
    Host(HostId),
}
```

Sub-entities (peers, channels, invoices) appear *inside* observations against
the parent target's collector, not as collector targets themselves.

## 5.4 Connection resolution

A `NodeRegistry` (`src/collectors/registry.rs`) resolves identity IDs to
connection details at sidecar startup. This cleanly separates **what is being
monitored** from **how to reach it**:

```rust
pub struct NodeRegistry {
    pub bitcoin_nodes: HashMap<BitcoinNodeId, BitcoinNodeConnection>,
    pub lnd_nodes: HashMap<LndNodeId, LndNodeConnection>,
    pub hosts: HashMap<HostId, HostConnection>,
}
```

with `BitcoinRpcAuth::{UserPass, CookieFile}`, optional ZMQ endpoint,
LND gRPC/REST endpoints, macaroons, and TLS cert paths.

## 5.5 Open questions

- Whether peer/channel/invoice IDs should be **scoped** under their parent
  node (e.g. `(LndNodeId, LndChannelId)`) — currently a `LndChannelId` is
  globally unique by string content, which works because LND uses chan_id
  scids but breaks if the sidecar ever monitors two LND nodes with overlapping
  legacy channel ID spaces.
- Whether `EntityRef::Sidecar(SidecarId)` should exist for heartbeat
  observations whose subject is the sidecar itself. Currently heartbeats
  appear to land on some entity per `ObservationContext` but the type doesn't
  enforce this.

---

# 6. Observations Domain

The richest and best-developed area of the codebase. Lives under
`src/observations/types/`.

## 6.1 The envelope

```rust
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
```

Constructor helpers exist for each payload variant
(`Observation::metric`, `::capability`, `::event`, `::heartbeat`, `::health`,
`::inventory`, `::state`, `::transition`) — `src/observations/types.rs:50-235`.

## 6.2 Source / provenance

```rust
pub struct ObservationSource {
    pub sidecar_id: SidecarId,
    pub collector: CollectorRef,
}

pub enum ObservationOrigin {
    Collected,
    Computed,
    Imported,
    UserReported,
}
```

> **Divergence from prior spec:** The prior spec proposed `ObservationSource
> { collector_id, collector_kind }`. The code records the full `CollectorRef`
> (id + integration kind + instance label) **and** the producing `SidecarId`,
> plus an `ObservationOrigin` axis distinguishing collected vs computed vs
> imported vs user-reported. This is a stronger provenance model than the
> prior spec required.

## 6.3 The payload — ten variants

Target shape after ADR-R2:

```rust
pub enum ObservationPayload {
    Capability(CapabilityObservation),
    Diagnosis(DiagnosisObservation),               // ADR-R2 (NEW)
    Event(EventObservation),
    Heartbeat(HeartbeatObservation),
    Health(HealthCheckObservation),
    IncidentSignal(IncidentSignalObservation),     // ADR-R2 (NEW)
    Inventory(InventoryObservation),
    Metric(MetricObservation),
    State(StateObservation),
    Transition(TransitionObservation),
}
```

Both `IncidentSignal` and `Diagnosis` types are already defined in
`src/observations/types/{incident_signal,diagnosis}.rs`; ADR-R2 promotes
them to first-class payload variants so they flow through the same
`Observation` envelope as collector-produced observations. The engine
(ADR-L4) produces `IncidentSignal` observations with
`ObservationOrigin::Computed` and a `CollectorRef` identifying the
incident engine; the ingestion path is the same as for primary
observations.

> **Divergence from prior spec (historical):** The prior spec listed four
> variants (Metric / State / Event / Health). The code had eight; ADR-R2
> brings it to ten. The additions are first-class types, not extensions
> of existing variants:
>
> - **Capability** — *can* Bithound monitor this thing right now?
> - **Heartbeat** — sidecar liveness with `monotonic_uptime_ms`, version,
>   and per-collector statuses.
> - **Inventory** — what *is* this entity? Static-ish facts.
> - **Transition** — explicit "X went from A to B."
> - **IncidentSignal / Diagnosis** — derived observations produced
>   by the incident engine / future diagnosis layer. **Promoted to
>   payload variants by ADR-R2.**

### 6.3.1 Metric

`MetricKind::{Gauge, Counter, Delta, Histogram, Summary}` with a
`validate()` method enforcing kind/value compatibility. `MetricValue` is
`Numeric(NumericValue) | Histogram(HistogramValue) | Summary(SummaryValue)`,
with `NumericValue::{U64, I64, F64}`. `Unit` includes Bitcoin-specific
denominations: `Satoshis`, `MilliSatoshis`, `WeightUnits`, `VirtualBytes`,
plus generic `Bytes`, `Seconds`, `Milliseconds`, `Count`, `Ratio`,
`Dimensionless`, `Custom(String)`.

### 6.3.2 State

> **Major divergence from prior spec.** The prior spec proposed
> `StateName + StateValue` (semi-typed) with optional typed decoders. The
> code is **strongly typed** instead:
>
> ```rust
> pub enum StateObservation {
>     BitcoinBlockchain(BitcoinBlockchainState),
>     BitcoinMempool(BitcoinMempoolState),
>     BitcoinNetwork(BitcoinNetworkState),
>     BitcoinPeerSummary(BitcoinPeerSummaryState),
>     LndNode(LndNodeState),
>     LndWallet(LndWalletState),
>     LndChannelSummary(LndChannelSummaryState),
>     Host(HostState),
> }
> ```
>
> Each variant is a fully-typed struct with the exact fields needed for the
> incident catalog. `StateName` and `StateValue` types are *defined* in
> `src/observations/types/state.rs:22-30` but are **not used** by
> `StateObservation` itself — they look like legacy or future-extensibility
> types.
>
> Concretely, `BitcoinBlockchainState` already maps to `getblockchaininfo`
> output (chain, blocks, headers, best_block_hash, verification_progress,
> initial_block_download, pruned, size_on_disk_bytes). `LndNodeState` already
> maps to `lnrpc.GetInfo`. This is the spec's "typed decoders" idea taken
> further: there is no decoding step — observations *are* the typed value.

### 6.3.3 Event

```rust
pub struct EventObservation {
    pub name: EventName,
    pub severity: EventSeverity,
    pub body: Option<String>,
}

pub enum EventSeverity { Debug, Info, Notice, Warning, Error, Critical }
```

> **Divergence from prior spec:** The prior spec proposed a free-form
> `attributes: serde_json::Value`. The code attaches structured attributes
> at the *observation envelope* level (`Attributes`) rather than per-event.
> Events carry a severity ordinal that the spec didn't have.

### 6.3.4 Health

`HealthCheckObservation` carries `target: HealthTargetId`, `status`,
`latency_ms`, `message`, and structured `HealthError { code, message,
retryable }`. `HealthStatus::{Ok, Warning, Critical, Unknown}` — note this
is a separate ordinal from `EventSeverity`.

### 6.3.5 Heartbeat

```rust
pub struct HeartbeatObservation {
    pub sequence: u64,
    pub sidecar_time: DateTime<Utc>,
    pub monotonic_uptime_ms: Option<u64>,
    pub sidecar_version: String,
    pub status: HeartbeatStatus,        // Alive | Degraded
    pub collector_statuses: Vec<CollectorStatus>,
}
```

Comment on `HeartbeatStatus` (`src/observations/types/health.rs:28-30`):
*"The application should compute Degraded from local component help. It
should NOT mark itself degraded merely because the monitored node is
unhealthy."* — a clear policy that sidecar health and monitored-node health
are distinct.

### 6.3.6 Capability

```rust
pub struct CapabilityObservation {
    pub capability: CapabilityName,
    pub status: CapabilityStatus,       // Available | Unavailable | Degraded | Unknown
    pub reason: Option<String>,
}
```

### 6.3.7 Inventory

`InventoryObservation { name: InventoryName, facts: BTreeMap<String,
InventoryValue> }` with `InventoryValue::{String, Bool, U64, I64, F64,
StringList}`. Bounded enum, no arbitrary JSON.

### 6.3.8 Transition

`TransitionObservation { name, from: StateAtom, to: StateAtom, reason }`.
Has a `validate()` method ensuring `from` and `to` are the same `StateAtom`
variant.

## 6.4 Attributes

```rust
pub struct Attributes(pub BTreeMap<String, AttributeValue>);

pub enum AttributeValue { String, Bool, I64, U64, F64 }
```

> **Divergence from prior spec:** Attribute values are bounded by an
> enum — explicitly *not* `serde_json::Value`. The comment in
> `src/observations/types.rs:305-307` documents the choice: "Attribute values
> should not be arbitrary JSON. Thus we keep them bounded through an enum."

## 6.5 Batches and probe runs

```rust
pub struct ObservationBatch {
    pub id: ObservationBatchId,
    pub collector: CollectorRef,
    pub sidecar_id: SidecarId,
    pub window: ProbeWindow,
    pub result: ProbeResult,
}

pub struct ProbeWindow { /* started_at..completed_at, enforces start ≤ end */ }

pub enum ProbeResult {
    Ok { observations: Vec<Observation> },
    Failed {
        health: HealthCheckObservation,
        partial_observations: Vec<Observation>,
        error: CollectionError,
    },
}
```

> **Divergence from prior spec:** The prior spec mused that a collector's
> output might be `Vec<Observation>` *or* a richer `CollectorRun`. The code
> chose the richer option and made one important guarantee: **a failed
> probe must carry a `HealthCheckObservation`** (`ProbeResult::Failed.health`
> is non-optional). Conversely, "successful probes never carry health" —
> health belongs in the observations list. This is the documented invariant
> in `src/observations/types.rs:286-289`.

## 6.6 Observation naming

Names are `String` newtypes, not enums or static consts:
- `MetricName(String)`
- `EventName(String)`
- `CapabilityName(String)`
- `InventoryName(String)`
- `TransitionName(String)`
- `SignalName(String)`
- `DiagnosisName(String)`
- `HealthTargetId(String)`

The convention in `INCIDENT_CATALOG.md` and the example doc-comments
suggests dotted namespaces like `bitcoin.zmq.rawtx`, `bitcoin.no_peers`.

## 6.7 Open questions still open

- Are observations stored append-only? *No storage yet — the question is
  unanswered.*
- Are `DiagnosisObservation` and `IncidentSignalObservation` intended to
  be added to `ObservationPayload`? They're defined and re-exported from
  `observations` but not in the enum.
- Is the orphan `StateValue` enum slated for deletion, or reserved for
  arbitrary state observations that don't fit the typed variants?

---

# 7. Collectors Domain

`src/collectors/{mod.rs, types.rs, traits.rs, registry.rs, error.rs}`

## 7.1 Status

| File | Status |
|---|---|
| `mod.rs` | Declares submodules, re-exports `types::*`. |
| `types.rs` | Fully developed types (200+ LOC). |
| `traits.rs` | **Empty (whitespace only)** — no `Collector` trait yet. |
| `registry.rs` | `NodeRegistry` + connection types. Fully developed. |
| `error.rs` | **Empty** — collection error lives in `types.rs` instead. |

## 7.2 Types

```rust
pub struct CollectorId(pub String);                         // in shared
pub struct CollectionRunId(pub Uuid);

pub struct CollectorDescriptor {
    pub id: CollectorId,
    pub integration: IntegrationKind,
    pub target: CollectorTarget,
    pub instance_label: String,
    pub description: Option<String>,
}

pub struct CollectorRef {                                   // small, hashable, serializable
    pub id: CollectorId,
    pub integration: IntegrationKind,
    pub instance_label: String,
}

pub enum CollectorSetup { Disabled, Enabled(CollectorDescriptor) }

pub enum IntegrationKind {
    BitcoinCoreRpc  { interval: Duration },
    BitcoinCoreZmq,
    LndGrpcPoll     { interval: Duration },
    LndGrpcStream,
    LndRest         { interval: Duration },
    Host            { interval: Duration },
}

pub enum CollectorMode { Polling, Subscription }   // derived from IntegrationKind
```

> **Divergence from prior spec:** The prior spec used a flat `CollectorKind`
> enum. The code's `IntegrationKind` **embeds scheduling intent in the type**
> — polling variants carry their own `interval`, subscription variants
> (`BitcoinCoreZmq`, `LndGrpcStream`) carry none. `IntegrationKind::mode()`
> and `interval()` decode this. No CLN or Elements variants yet.

## 7.3 The Collector traits

`src/collectors/traits.rs` is empty in code. Target shape per ADR-C1
and ADR-C2:

```rust
#[async_trait]
pub trait PollingCollector: Send + Sync {
    fn descriptor(&self) -> &CollectorDescriptor;

    /// Run one collection pass. Returns a batch whose ProbeResult
    /// encodes success or failure; never returns Err.
    async fn poll(&self, ctx: CollectionContext) -> ObservationBatch;
}

#[async_trait]
pub trait SubscriptionCollector: Send + Sync {
    fn descriptor(&self) -> &CollectorDescriptor;

    /// Run until the subscription dies or the sink is closed. Returns
    /// Err if the connection died unrecoverably.
    async fn run(&self, ctx: CollectionContext, sink: BatchSink)
        -> Result<(), CollectionError>;
}

pub struct BatchSink { /* mpsc::Sender<ObservationBatch> internally */ }

impl BatchSink {
    pub async fn send(&self, batch: ObservationBatch) -> Result<(), SinkError>;
}

pub enum SinkError { Closed }
```

**Contract for `PollingCollector::poll`:**

1. Never panics.
2. Never returns `Err` (no `Result`).
3. Every internal error is mapped to a `CollectionError` and wrapped
   into `ProbeResult::Failed` with a `HealthCheckObservation`.
4. Observations collected before a failure are preserved in
   `ProbeResult::Failed.partial_observations`.

V0 implements only `PollingCollector` concretely. `SubscriptionCollector`
is defined so the runtime ADR cluster has a shape to target;
`BitcoinCoreZmqCollector` / `LndGrpcStreamCollector` ship in V0.1+.

## 7.4 Collection context

```rust
pub struct CollectionContext {
    pub sidecar_id: SidecarId,           // ADR-C3 §C3.1 (NEW)
    pub collector_id: CollectorId,
    pub target: CollectorTarget,
    pub now: DateTime<Utc>,
    pub run_id: CollectionRunId,
}
```

The new `sidecar_id` field lets the collector stamp
`ObservationBatch.sidecar_id` and `ObservationSource.sidecar_id` on
emitted observations without holding its own copy. The runtime owns
sidecar identity in one place.

## 7.5 Errors

```rust
pub struct CollectionError {
    pub kind: CollectionErrorKind,
    pub message: String,
}

pub enum CollectionErrorKind {
    Unreachable, Timeout, AuthenticationFailed, PermissionDenied,
    ProtocolError, DecodeError, InvalidResponse, RateLimited,
    Misconfigured, UnsupportedVersion, Internal,
}

impl CollectionErrorKind {
    pub fn is_retryable(&self) -> bool { /* Unreachable | Timeout | RateLimited | Internal */ }
}
```

The `is_retryable` method is a contract the scheduler will need.

**Mapping `RpcError → CollectionErrorKind`** for the Bitcoin Core RPC
client (per ADR-C3 §C3.8):

| `RpcError` | `CollectionErrorKind` |
|---|---|
| `Network(_)` | `Unreachable` |
| `Timeout` | `Timeout` |
| `Auth` | `AuthenticationFailed` |
| `HttpStatus(_)` | `ProtocolError` |
| `BitcoindError { … }` | `InvalidResponse` |
| `Decode(_)` | `DecodeError` |

## 7.6 Connection registry & shared HTTP client

`NodeRegistry` resolves IDs → connection structs at startup (see § 5.4).
Connection secrets use `secrecy::SecretString`.

Per ADR-C3 §C3.6, **one `reqwest::Client` per sidecar process** is
created by the runtime and injected into every HTTP-using collector
via its constructor. Connection pooling matters; per-collector clients
waste TCP connections.

## 7.7 Concrete collectors

| Collector | Mode | Trait | V0? | Module |
|---|---|---|---|---|
| `BitcoinCoreRpcCollector` | Polling | `PollingCollector` | **Yes** | `src/collectors/bitcoin_core/rpc.rs` |
| `BitcoinCoreZmqCollector` | Subscription | `SubscriptionCollector` | V0.1+ | `src/collectors/bitcoin_core/zmq.rs` |
| `LndGrpcPollCollector` | Polling | `PollingCollector` | V0.1+ | `src/collectors/lnd/grpc_poll.rs` |
| `LndGrpcStreamCollector` | Subscription | `SubscriptionCollector` | V0.1+ | `src/collectors/lnd/grpc_stream.rs` |
| `LndRestCollector` | Polling | `PollingCollector` | V0.1+ | `src/collectors/lnd/rest.rs` |
| `HostCollector` | Polling | `PollingCollector` | V0.1+ | `src/collectors/host/mod.rs` |

### 7.7.1 `BitcoinCoreRpcCollector` (V0)

Per ADR-C3 §C3.4. Four RPCs per poll, in order:

| RPC | State observation | Health observation | Diagnostic rules served |
|---|---|---|---|
| `getblockchaininfo` | `BitcoinBlockchainState`  | `bitcoin.rpc.getblockchaininfo` | A1, A2, IBD progress |
| `getmempoolinfo`    | `BitcoinMempoolState`     | `bitcoin.rpc.getmempoolinfo`    | A4 mempool full, minrelayfee |
| `getnetworkinfo`    | `BitcoinNetworkState`     | `bitcoin.rpc.getnetworkinfo`    | A3 networkactive, connections |
| `getpeerinfo`       | `BitcoinPeerSummaryState` | `bitcoin.rpc.getpeerinfo`       | A3 peer count |

Per RPC success, the collector emits one state observation plus one
`HealthStatus::Ok` health observation. On RPC failure, the batch is
returned as `ProbeResult::Failed` with `partial_observations`
containing whatever succeeded before the failure.

**Construction.** Per ADR-C3 §§C3.2, C3.5:

```rust
impl BitcoinCoreRpcCollector {
    pub fn new(
        descriptor: CollectorDescriptor,
        connection: BitcoinNodeConnection,
        http: reqwest::Client,
        config: BitcoinCoreRpcCollectorConfig,
    ) -> Result<Self, BuildError>;
}

pub struct BitcoinCoreRpcCollectorConfig {
    pub timeout_per_rpc: Duration,    // default 5s (ADR-C3 §C3.7)
}
```

The constructor validates shape (URL parses, auth well-formed) but
does **not** ping the network. Unreachability surfaces as a
`ProbeResult::Failed` on the first poll, not as a startup error.

**RPC client.** Per ADR-C3 §C3.8, `BitcoinRpcClient` is a thin in-crate
JSON-RPC wrapper over `reqwest::Client` — ~150 LOC, no third-party
RPC dependency:

```rust
// src/collectors/bitcoin_core/rpc_client.rs
pub struct BitcoinRpcClient {
    url: String,
    auth: BitcoinRpcAuth,
    http: reqwest::Client,
    timeout: Duration,
}

impl BitcoinRpcClient {
    pub async fn get_blockchain_info(&self) -> Result<GetBlockchainInfoResponse, RpcError>;
    pub async fn get_mempool_info(&self)    -> Result<GetMempoolInfoResponse, RpcError>;
    pub async fn get_network_info(&self)    -> Result<GetNetworkInfoResponse, RpcError>;
    pub async fn get_peer_info(&self)       -> Result<GetPeerInfoResponse, RpcError>;
}
```

Each call is wrapped in `tokio::time::timeout(self.timeout, …)`;
worst-case `poll` upper bound is N × `timeout_per_rpc` (4 × 5s = 20s
by default). V0.1+ may swap to `bitcoincore-rpc-async` if the surface
grows.

## 7.8 Open questions

- **Trait shape.** **Resolved by ADR-C1**: two traits, `PollingCollector`
  and `SubscriptionCollector`.
- **Polling output shape.** **Resolved by ADR-C2**: `ObservationBatch`
  directly, no outer `Result`.
- **One collector per (integration × target) vs spanning?** Confirmed
  one per (integration × target), as `CollectorDescriptor.target`
  implies. The runtime instantiates one collector per descriptor.
- **Scheduler shape** (polling intervals, subscription restart with
  backoff, single-tick loop vs N tasks) — still open, next ADR cluster.
- **Connection refresh.** `BitcoinRpcAuth::CookieFile` cookies rotate
  when bitcoind restarts. V0 reads the file on each request; if FS load
  becomes a concern, add a short in-memory cache. Not designed yet.

---

# 8. Read Models / Projections Domain

`src/read_models/`

## 8.1 The wrapper

```rust
pub struct Projected<T> {
    pub value: T,
    pub observation_id: ObservationId,
    pub observed_at: DateTime<Utc>,
}
```

Every read-model value carries an evidence pointer (the observation that
contributed it) and the wall-clock observed time. This is what makes
diagnostics able to attach `EvidenceRef`s without re-querying.

## 8.2 The traits

Six traits, one per observation payload type that has a queryable
projection. **All traits are generic over their payload type** per
ADR-R1 §R1.1: sub-variants (e.g. `BitcoinBlockchainState`,
`LndNodeState`) are collector-side concerns, not read-model-side.
Consumers pattern-match the returned enum where needed.

```rust
trait StateReadModel: Send + Sync + Debug {                                 // ADR-R1 (rewritten)
    fn latest_state(&self, subject: &EntityRef, name: &StateName)
        -> Option<Projected<StateObservation>>;
    fn states_for(&self, subject: &EntityRef)
        -> Vec<Projected<StateObservation>>;
}

trait MetricReadModel: Send + Sync + Debug {
    fn latest_metric        (&self, &EntityRef, &MetricName)             -> Option<Projected<MetricObservation>>;
    fn metric_samples_since (&self, &EntityRef, &MetricName, since: Utc) -> Vec<Projected<MetricObservation>>;
    fn unchanged_for        (&self, &EntityRef, &MetricName)             -> Option<Vec<Projected<MetricObservation>>>;
}

trait HealthReadModel:        fn current_health(&EntityRef, &HealthTargetId) -> Option<…>;
trait CapabilityReadModel:    fn current_capability / capabilities_for(&EntityRef);
trait HeartbeatReadModel:     fn latest_heartbeat / heartbeats_since(since);
trait IncidentSignalReadModel:
    fn current_signal(&EntityRef, &SignalName),
    fn active_signals_for(&EntityRef),
    fn active_signals_for_incident_kind(&EntityRef, &IncidentKind);
```

Each `StateObservation` variant has a canonical `StateName` accessible
via `StateObservation::name()` and exposed as `&'static str` constants
in `src/observations/types/state/well_known.rs` (ADR-R1 §R1.2).

**Optional typed helpers** live in a separate `StateReadModelExt`
auto-implemented for any `StateReadModel` (ADR-R1 §R1.3) — rules use
them when convenient, or query generically and pattern-match.

## 8.3 Concrete implementations

`ReadModelStore` (designed in ADR-R1 §R1.4; not yet in code) implements
all six traits in a single concrete struct. Lives in
`src/read_models/store.rs`.

## 8.4 Notes

- `MetricReadModel::unchanged_for` is for stale-metric detection (e.g. A1
  tip-lag, A2 IBD stall — `verificationprogress` flat over time).
- `IncidentSignalReadModel::active_signals_for_incident_kind` is what
  the incident engine (ADR-L4) uses when reasoning across signals
  contributing to the same incident kind.
- Read-model traits are **query-only**. The update path
  (`ReadModelStore::apply(&mut self, &Observation)`) is **not** part
  of any trait — it's a method on the concrete store (ADR-R3 §1).

> **Divergence from prior spec (historical):** The prior spec defined a
> single `Projector { interested_in, apply }` trait plus a generic
> `ProjectionStore<P>`. The code keeps query-only traits per
> observation family; the apply path lives on the concrete store, not
> in a separate `Projector` taxonomy (ADR-R1 §R1.4).

## 8.5 Read-model store (designed; not yet in code)

Per ADR-R1 §R1.4. The store is **a thin assembler over six per-type
projections**, each in its own module under
`src/read_models/projections/`:

```rust
// src/read_models/projections/mod.rs
pub mod state;
pub mod metric;
pub mod health;
pub mod capability;
pub mod heartbeat;
pub mod incident_signal;

pub trait Projection: Send + Sync + std::fmt::Debug {
    fn apply(&mut self, obs: &Observation) -> Result<(), ProjectionError>;
}

pub enum ProjectionError {
    InvalidPayload(String),
    InternalConsistency(String),
}

// src/read_models/store.rs
pub struct ReadModelStoreConfig {
    pub metric_history_capacity: usize,        // default 1000 (ADR-R3 §4)
    pub heartbeat_history_capacity: usize,     // default 256
}

pub struct ReadModelStore {
    pub state:           projections::state::StateProjection,
    pub metric:          projections::metric::MetricProjection,
    pub health:          projections::health::HealthProjection,
    pub capability:      projections::capability::CapabilityProjection,
    pub heartbeat:       projections::heartbeat::HeartbeatProjection,
    pub incident_signal: projections::incident_signal::IncidentSignalProjection,
}

pub enum ApplyError { Projection(ProjectionError) }

impl ReadModelStore {
    pub fn new(config: ReadModelStoreConfig) -> Self;
    pub fn apply(&mut self, obs: &Observation) -> Result<(), ApplyError>;
}
```

`apply` dispatches on `obs.payload`:
`State → state`, `Metric → metric`, `Health → health`,
`Capability → capability`, `Heartbeat → heartbeat`,
`IncidentSignal → incident_signal`.
`Event`, `Inventory`, `Transition`, and `Diagnosis` are **no-ops at
the projection layer** in V0 (ADR-R3 §3); they're still persisted to
the observation store via a separate concern.

The store is **synchronous, single-writer** (`&mut self`, no locks)
per ADR-R3 §2. The runtime loop owns the store and serializes
apply + queries within each tick. V0.1+ may wrap with
`Arc<RwLock<…>>` or move to arc-swap snapshots without changing the
trait surface.

## 8.6 Contributor story

Per ADR-R1 §R1.5:

- **New collector emits an existing state shape** → zero read-model changes.
- **New state shape** (e.g. `ClnNodeState`) → one variant on
  `StateObservation`, one entry in `StateObservation::name()`, one
  `well_known` constant. `StateProjection` handles it generically.
  Optionally add a typed helper to `StateReadModelExt`.
- **New observation type entirely** (e.g. `TraceObservation`) → payload
  variant + new projection module + new trait + one line in
  `DiagnosticContext`. Heaviest case; correctly central.

## 8.7 Open questions

- **Where does state mutation happen?** **Resolved by ADR-R1 §R1.4 +
  ADR-R3 §1**: a single concrete `ReadModelStore` exposes
  `apply(&mut self, &Observation)` as a method (not yet a trait).
- **Is `LndChannelSummary` per-node aggregate, or are per-channel
  projections planned separately?** Still open. Current `StateObservation`
  variant is the per-node aggregate. A future per-channel state shape
  would be added the same way any state variant is added (ADR-R1 §R1.5).
- **Cold start vs replay.** **Resolved by ADR-R3 §5**: cold start in V0;
  add `ReadModelStore::restore_from(impl Iterator<Item = Observation>)`
  in V0.1, no architectural change.

---

# 9. Diagnostics Domain

`src/diagnostics/{mod.rs, traits.rs, types.rs}`

## 9.1 Module status

`mod.rs` declares `mod traits; mod types;` without `pub` — the diagnostics
module's types are **not currently exported** from the crate. This is
probably a stub state, since other modules (read_models, observations)
do export their `traits`/`types` modules. **Resolved by ADR-001 §3**:
the submodules become `pub`.

## 9.2 Types

The target shape, after ADR-001 §4 and ADR-L1 land:

```rust
pub struct DiagnosticContext<'a> {
    pub now: DateTime<Utc>,
    pub subject: &'a EntityRef,
    pub state:        &'a dyn StateReadModel,
    pub metrics:      &'a dyn MetricReadModel,
    pub health:       &'a dyn HealthReadModel,
    pub capabilities: &'a dyn CapabilityReadModel,
    pub heartbeats:   &'a dyn HeartbeatReadModel,
    pub signals:      &'a dyn IncidentSignalReadModel,   // ADR-001 §4
}

pub struct IncidentSignalDraft {
    pub subject: EntityRef,
    pub signal: SignalName,
    pub kind: IncidentKind,                              // ADR-L1
    pub dimension: Option<String>,                       // ADR-L1
    pub severity: SignalSeverity,
    pub status: SignalStatus,           // Active | Cleared
    pub confidence: Confidence,         // Low | Medium | High
    pub evidence: Vec<EvidenceRef>,
}

pub trait DiagnosticRule {
    fn id(&self) -> &'static str;
    fn evaluate(&self, ctx: DiagnosticContext<'_>) -> anyhow::Result<Vec<IncidentSignalDraft>>;
}
```

**Rules own hysteresis.** Per ADR-L2 §L2.1, rules look back through
read models (`MetricReadModel::metric_samples_since`, `unchanged_for`,
`StateReadModel::*`, and the new `signals: &dyn IncidentSignalReadModel`)
and only emit `Active` once they're sure the condition is real; only
emit `Cleared` once they're sure it has ended. The engine treats every
Active as immediate-open and every Cleared as immediate-resolve.

**Rules declare their incident kind per draft.** The rule sets
`kind: IncidentKind` on each draft (referencing
`well_known::*` constants for built-in kinds, or any string for
user-config kinds). The engine validates `(subject, kind, dimension)`
against the `KindRegistry` (ADR-L1 §4).

> **Divergence from prior spec (historical):**
>
> - `DiagnosticRule::id` returns `&'static str`, not a dedicated
>   `DiagnosticRuleId` newtype.
> - The rule output is `IncidentSignalDraft`, not `DiagnosticFinding`.
>   A draft has both an **active/cleared status** and a **confidence**.
> - `Evidence` is `EvidenceRef(ObservationId)`, not a polymorphic enum.
> - The original code's `DiagnosticContext` did not carry
>   `IncidentSignalReadModel` — ADR-001 §4 adds it.
> - The original code's `IncidentSignalDraft` did not carry
>   `kind` or `dimension` — ADR-L1 adds them.

## 9.3 Concrete rules

**None implemented.** The `INCIDENT_CATALOG.md` document at
`docs/INCIDENT_CATALOG.md` enumerates ~17 candidate rules across three
categories (Bitcoin Core A1–A8, LND B1–B6, Elements C1–C3, plus X1/X2).
This is clearly the source-of-truth design doc for what diagnostics need
to detect. The catalog documents for each incident: symptom, signals,
diagnosis text, recommended action, false positives, and citations.

## 9.4 Open questions

- **Are rules stateless across evaluations?** **Resolved by ADR-L2 §L2.1**:
  rules are stateless by trait signature. They reconstruct window state
  from read models each evaluation; the engine does not provide
  hysteresis. A future `Hysteresis<T>` helper module can be added if
  boilerplate becomes a problem.
- **Should `DiagnosticContext` carry `IncidentSignalReadModel`?**
  **Resolved by ADR-001 §4**: yes, added as the `signals` field.
- **How is the rule registry assembled and scheduled?** Still open.
  Decision deferred to the runtime ADR cluster (post-L5).

---

# 10. Incidents Domain

`src/incidents/{mod.rs, types.rs}`

## 10.1 The aggregate

Target shape after ADR-001 §1, §2 and ADR-L1 land:

```rust
pub struct Incident {
    pub id: IncidentId,
    pub fingerprint: IncidentFingerprint,                // ADR-L1 (NEW)
    pub kind: IncidentKind,                              // newtype around String
    pub subject: EntityRef,
    pub severity: IncidentSeverity,                      // Info | Warning | Critical (monotonic max — ADR-L3)
    pub status: IncidentStatus,                          // Open | Acknowledged | Resolved | Suppressed
    pub opened_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub signal_observation_ids: Vec<ObservationId>,      // ADR-001 §2 (was Option<ObservationId>)
    pub evidence: Vec<EvidenceRef>,
    pub summary: String,
    pub evidence_summary: Vec<String>,                   // durable display copy for resolved retention
}
```

`IncidentStatus::Suppressed` is **reserved for V0.2** (per ADR-L5 §L5.2);
the V0.1 engine does not set it. V0.1 suppression is notifier-side and
does not mutate incident state. See § 10.7.

> **Divergence from prior spec (historical):**
>
> - **`IncidentKind` stays as `pub struct IncidentKind(pub String)`**, not
>   an enum. The string-newtype is preserved with a `well_known.rs`
>   constants module for canonical V0 kinds (ADR-L1 §5).
> - **`evidence_summary: Vec<String>`** is a durable human-readable
>   redundant copy of evidence so a long-resolved incident can still
>   display useful context even if underlying observations are pruned.
>   This is a thoughtful addition the prior spec missed.
> - The original code had no `fingerprint` field and a singular
>   `signal_observation_ids: Option<ObservationId>`. **Resolved by
>   ADR-L1 (fingerprint added) and ADR-001 §2 (Vec).**
> - The original code had `IncidentStatus::Supressed` typo. **Resolved
>   by ADR-001 §1.**

## 10.2 Lifecycle event

```rust
pub enum IncidentLifecycleEvent {
    Opened(Incident),
    Escalated {
        incident: Incident,
        previous_severity: IncidentSeverity,
        new_severity: IncidentSeverity,
    },
    Resolved(Incident),
}

pub enum IncidentNotificationEventKind { Opened, Escalated, Resolved }
```

**Escalated fires only on strict severity increase** (ADR-L3 §L3.2).
Per ADR-L3 §L3.1, incident severity is monotonic max — once Critical,
always Critical for the rest of that incident's lifetime — so
de-escalation within an incident is structurally impossible.
Severity downgrade only happens across an Opened/Resolved/new-incident
boundary (ADR-L2 §L2.3).

> **Divergence from prior spec (historical):** The prior spec used
> `Updated(Incident)` as the middle variant. The code uses `Escalated`,
> which is narrower and more meaningful — and per ADR-L3, fires only
> on strict severity increase.

`IncidentLifecycleEvent` exposes helpers:

- `notification_kind() -> IncidentNotificationEventKind`
- `incident() -> &Incident`

## 10.3 What does *not* exist in code yet (but is designed)

As of 2026-05-17 these are specified in ADRs L1–L5 but not implemented:

- `IncidentFingerprint` (ADR-L1).
- `IncidentCommand` enum with `RecordSignal` only for V0 (ADR-L4 §L4.3).
- `IncidentEngine` struct with `handle()` returning `HandleOutcome`
  (ADR-L4 §§L4.1–L4.2).
- `IncidentRepository` trait (ADR-L4 §L4.6).
- `KindRegistry` + `IncidentKindSpec` + `EntitySubjectKind` (ADR-L1 §§3–4).
- `ActorId(String)` strawman (ADR-L5 §L5.5).
- `SuppressionRule` + `SuppressionRepository` (ADR-L5 §L5.3) — V0.1.

The incident domain remains **a data shape and lifecycle event tag in
code**, but the surface around it is now fully specified — implementation
is the next gate.

## 10.4 Open questions

- **What is the dedup key?** **Resolved by ADR-L1**:
  `(subject, kind, dimension: Option<String>)`, computed by the engine
  on receipt and validated against `KindRegistry`.
- **Does Escalated fire on de-escalation?** **Resolved by ADR-L3
  §§L3.1–L3.2**: no. Severity is monotonic max; `Escalated` fires only
  on strict increase.
- **Is `IncidentStatus::Acknowledged` part of V0?** **Resolved by
  ADR-L4 §L4.3**: no. The variant stays in the type for V0.2; no V0
  command sets it.
- **The `Supressed` typo.** **Resolved by ADR-001 §1**: renamed to
  `Suppressed`.
- **Suppressed semantics.** **Resolved by ADR-L5**: notifier-side
  filtering via `SuppressionRule`; `IncidentStatus::Suppressed` stays
  vestigial in V0.1; reserved for operator-acknowledged-known in V0.2.
- **Single-writer guarantees / reconciliation.** **Resolved by ADR-L4
  §L4.4**: no periodic reconciliation. Runtime enforces single-writer
  via write-through retry.

## 10.5 Incident engine (designed; not implemented)

Per ADR-L4, ADR-D1, ADR-D3, and ADR-D4. The engine lives in
`src/incidents/engine.rs`, holds open incidents in an in-memory map
keyed by `IncidentFingerprint`, and is the single mutator of incident
state in V0.

```rust
pub struct IncidentEngine {
    kinds: KindRegistry,
    sidecar_id: SidecarId,
    open_incidents: HashMap<IncidentFingerprint, Incident>,
}

// ADR-D3: full command vocabulary; Acknowledge/Resolve stubbed in V0/V0.1.
pub enum IncidentCommand {
    RecordSignal(UnvalidatedIncidentSignalDraft),
    Acknowledge { id: IncidentId, by: ActorId, at: DateTime<Utc> },
    Resolve     { id: IncidentId, by: ActorId, at: DateTime<Utc>, reason: String },
}

// ADR-D4: events-only output (β). HandleOutcome was removed.
pub enum IncidentEvent {
    SignalRecorded(Observation),                       // → observation store
    IncidentTouched(Incident),                         // → incident repo
    Lifecycle(IncidentLifecycleEvent),                 // → Notifier
    DraftRejected { rule_id: String, error: DraftError },
    DraftBelowConfidenceFloor {
        kind: IncidentKind, confidence: Confidence, floor: Confidence,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("draft validation: {0:?}")] Draft(DraftError),
    #[error("command not yet implemented: {0}")] NotYetImplemented(&'static str),
}

impl IncidentEngine {
    pub fn new(kinds: KindRegistry, sidecar_id: SidecarId,
               open_incidents: Vec<Incident>) -> Self;

    pub fn handle(&mut self, cmd: IncidentCommand, now: DateTime<Utc>)
        -> Result<Vec<IncidentEvent>, EngineError>;
}
```

### 10.5.1 Receive-a-draft flow

The canonical `RecordSignal` flow, combining ADRs L1, L2, L3, D1, D4:

```text
unvalidated draft arrives in IncidentCommand::RecordSignal(_)
  ↓
KindRegistry::validate(unvalidated) → ValidatedIncidentSignalDraft
  rejected ──→ return Err(EngineError::Draft(_)) — emit no events
  ok
  ↓
compute fingerprint = (subject, kind, dimension)
  ↓
build IncidentSignalObservation (sidecar_id, ObservationOrigin::Computed)
emit IncidentEvent::SignalRecorded(observation)              ← first
  ↓
look up current Incident for this fingerprint
  ↓
draft.status = Active:
  draft.confidence < kind_spec.min_open_confidence:
    emit IncidentEvent::DraftBelowConfidenceFloor { … }
    (no IncidentTouched, no Lifecycle)

  no Incident, OR previous is Resolved (per ADR-L2 §L2.3):
    create new Incident; severity = draft.severity; status = Open
    emit IncidentEvent::IncidentTouched(incident)            ← second
    emit IncidentEvent::Lifecycle(Opened(incident))          ← third

  Open Incident exists:
    append evidence; new_sev = MAX(incident.sev, draft.sev) — ADR-L3
    if new_sev > old_sev:
      emit IncidentEvent::IncidentTouched(incident)
      emit IncidentEvent::Lifecycle(Escalated{previous, new})
    else:
      emit IncidentEvent::IncidentTouched(incident)
      (silent updated_at bump, no Lifecycle event — ADR-L3 §L3.3)

draft.status = Cleared:
  no Open Incident:           emit nothing past SignalRecorded
  Open Incident exists:       status = Resolved, resolved_at = now
                              emit IncidentEvent::IncidentTouched(incident)
                              emit IncidentEvent::Lifecycle(Resolved(incident))
```

**Event-ordering invariant (ADR-D4):** within a single `handle()` call,
events are emitted in side-effect order: `SignalRecorded` → `IncidentTouched`
→ `Lifecycle`. The runtime iterates events sequentially and trusts the
order. A unit-tested invariant.

### 10.5.2 Caller responsibilities (runtime loop)

The engine is pure; the runtime loop owns I/O. Per ADR-L4 §L4.4 and
ADR-D4 (event-driven dispatch):

```text
1. Startup
   - kinds = KindRegistry::load(user_config_path)
   - open  = incident_repo.load_open().await
   - engine = IncidentEngine::new(kinds, sidecar_id, open)

2. Per draft (unvalidated, from a rule)
   - events = engine.handle(RecordSignal(unvalidated_draft), now)?
   - for event in events:
       match event {
         SignalRecorded(obs)  → observation_store.append(&obs).await;
                                read_models.apply(&obs)?
         IncidentTouched(inc) → incident_repo.save(&inc).await    ← write-through
         Lifecycle(ev)        → notifier.dispatch(&ev, &compose(&ev)).await
         DraftRejected{…}     → tracing::warn!(…)
         DraftBelowConfidenceFloor{…} → tracing::debug!(…)
       }

3. Repo write failure
   - retry with backoff
   - if exhausted: rollback engine state (future) and surface the error
   - never: skip persistence and proceed to notify

4. Cloud sync (future V1.0+)
   - the same Vec<IncidentEvent> is pushed to the cloud control plane;
     no derivation step (per ADR-D4 cloud-readiness rationale)
```

The "never skip persistence" rule is what makes the no-reconciliation
choice safe.

## 10.6 Kind registry (designed; not implemented)

Per ADR-L1. Lives in `src/incidents/kinds.rs`.

```rust
pub struct IncidentKindSpec {
    pub name: String,
    pub allowed_subjects: Vec<EntitySubjectKind>,
    pub allows_dimension: bool,
    pub dimension_label: Option<String>,   // documentation-only
    pub min_open_confidence: Confidence,   // ADR-L2 §L2.2; default Medium
    pub source: KindSource,
}

pub enum KindSource { Builtin, UserConfig }

pub struct KindRegistry { kinds: HashMap<IncidentKind, IncidentKindSpec> }

impl KindRegistry {
    pub fn load(user_config: Option<&Path>) -> Result<Self, RegistryError>;
    pub fn lookup(&self, kind: &IncidentKind) -> Option<&IncidentKindSpec>;
    pub fn validate_draft(&self, draft: &IncidentSignalDraft) -> Result<(), DraftError>;
}

pub enum RegistryError {
    InvalidToml(String),
    DuplicateKind(IncidentKind),
    CannotOverrideBuiltin(IncidentKind),
    UnknownSubjectKind(String),
}

pub enum DraftError {
    UnknownKind(IncidentKind),
    DisallowedSubject {
        kind: IncidentKind,
        subject: EntitySubjectKind,
        allowed: Vec<EntitySubjectKind>,
    },
    DimensionRequired(IncidentKind),
    DimensionForbidden(IncidentKind),
}
```

Built-in defaults are embedded via
`include_str!("../../config/default_kinds.toml")`. A unit test enforces
parity between the TOML and `src/incidents/well_known.rs` constants.

Operator extensions are additive only (ADR-L1 §4); attempting to
override a built-in kind produces `RegistryError::CannotOverrideBuiltin`.

```toml
# config/default_kinds.toml — embedded in the binary
[[kinds]]
name = "bitcoin.tip_lag"
allowed_subjects = ["BitcoinNode"]
allows_dimension = false
min_open_confidence = "Medium"

[[kinds]]
name = "lnd.htlc_stuck"
allowed_subjects = ["LndChannel"]
allows_dimension = true
dimension_label = "payment_hash"

[[kinds]]
name = "host.disk_exhaustion"
allowed_subjects = ["Host"]
allows_dimension = true
dimension_label = "mount_path"
```

## 10.7 Suppression (designed; V0.1 — not in V0)

Per ADR-L5. Lives in `src/incidents/suppression.rs`.

```rust
pub struct SuppressionRuleId(pub Uuid);

pub struct SuppressionRule {
    pub id: SuppressionRuleId,
    pub fingerprint: IncidentFingerprint,
    pub until: Option<DateTime<Utc>>,     // None = indefinite
    pub reason: String,
    pub by: ActorId,                      // ActorId::system() for maintenance windows
    pub created_at: DateTime<Utc>,
}

#[async_trait]
pub trait SuppressionRepository: Send + Sync {
    async fn list_active(&self, now: DateTime<Utc>) -> Result<Vec<SuppressionRule>, RepoError>;
    async fn matches(&self, fingerprint: &IncidentFingerprint, now: DateTime<Utc>)
        -> Result<Option<SuppressionRuleId>, RepoError>;
    async fn add(&self, rule: SuppressionRule) -> Result<(), RepoError>;
    async fn remove(&self, id: SuppressionRuleId) -> Result<(), RepoError>;
}
```

Rules are **per-fingerprint only.** Maintenance-window TOML is expanded
into N per-fingerprint rules at config-load time against the catalog
of active incidents at window start (ADR-L5 §L5.1).

Suppression is **notifier-side** — the engine ignores rules entirely
(ADR-L5 §L5.2). See § 11.bis below for the dispatch-time integration.

---

# 11. Notifications Domain

`src/notifications/{mod.rs, types.rs, traits.rs, orchestrator.rs, targets/}`

The most complete module in the codebase. Types are stable; senders are
stubs that always return `PermanentError::BadRequest("not yet implemented")`.

## 11.1 Rules & matching

```rust
pub struct NotificationRule {
    pub id: NotificationRuleId,
    pub name: NotificationRuleName,
    pub enabled: bool,
    pub min_severity: IncidentSeverity,
    pub event_kinds: Vec<IncidentKind>,                  // empty = match all kinds
    pub target: NotificationTarget,
}

impl NotificationRule {
    pub fn matches(&self, event: &IncidentLifecycleEvent) -> bool { /* … */ }
}
```

`matches()` (orchestrator.rs:35-42 / types.rs:151-162) checks:
1. `enabled`
2. `event_kinds.is_empty() || event_kinds.contains(&incident.kind)`
3. `severity_at_least(incident.severity, min_severity)`

Severity rank: `Info < Warning < Critical`.

> **Divergence from prior spec:** The prior spec proposed a single
> `NotificationSubscription` keyed by sink kind. The code separates rules
> (severity + kind filter + which target) from per-sink subscription state
> (`TelegramSubscription`, `DiscordSubscription`). Cleaner.

## 11.2 Targets

```rust
pub enum NotificationTarget {
    #[cfg(debug_assertions)] Stdout,
    Discord(DiscordTarget),
    Telegram(TelegramTarget),
    Webhook(WebhookTarget),
}
```

> **Divergence from prior spec:** Stdout is **debug-only** (`cfg(debug_assertions)`).
> The prior spec made it a V0 sink. The code treats it as a developer
> convenience rather than a production option.

## 11.3 Message

```rust
pub struct NotificationMessage {
    pub incident_lifecycle_event: IncidentLifecycleEvent,
    pub title: String,
    pub summary: String,
    pub affected_component: Option<String>,
    pub diagnostic_summary: Option<String>,
    pub occurred_at: DateTime<Utc>,
}
```

The message is composed *upstream* of the orchestrator and passed in
alongside the lifecycle event. The orchestrator does not derive titles/
summaries from incidents itself.

## 11.4 The orchestrator

```rust
pub struct Notifier { rules, telegram: Option<TelegramService>,
                      discord: Option<DiscordService>, webhook: WebhookSender }

impl Notifier {
    pub async fn dispatch(&self, event: &IncidentLifecycleEvent,
                          message: &NotificationMessage)
        -> Vec<(NotificationRuleId, DeliveryReceipt)>;
}
```

`dispatch` filters rules with `matches`, then concurrently (`join_all`)
fans out to the appropriate sender per `NotificationTarget`. When a target
references a service that wasn't configured (e.g. a Telegram rule with no
TelegramService), the receipt is `PermanentError::NotConfigured`.

## 11.5 Delivery outcome taxonomy

```rust
pub enum DeliveryOutcome {
    Delivered { external_ref: Option<ExternalMessageRef> },
    Transient { error: TransientError, retry_after: Option<Duration> },
    Permanent { error: PermanentError },
    Suppressed { rule_id: SuppressionRuleId },         // ADR-L5 §L5.4 (V0.1)
}

pub enum TransientError {
    RateLimited, Network, Upstream5xx { status: u16 }, Unknown { detail: String },
}

pub enum PermanentError {
    AuthFailure, DestinationGone, BadRequest { detail }, NotConfigured,
}

pub enum ExternalMessageRef {
    Telegram { chat_id, message_id: i64 },
    Discord  { channel_id, message_id: u64 },
}
```

`Suppressed { rule_id }` is added in V0.1 per ADR-L5 §L5.4. It is not
a failure — the delivery never happened by design. Its presence in the
outcome enum gives the future operator UI a single shape to render
("muted by rule X") and keeps the rule-matching tally complete: every
`NotificationRule` that *would* have fired still appears in the
dispatch result with a Suppressed receipt.

> **Divergence from prior spec (historical):** Far richer than the prior
> spec's vague notification-delivery sketch. The Transient vs Permanent
> split with per-class enums is exactly what a retry scheduler needs.
> `external_ref` enables follow-up actions (edits, reactions) on
> Telegram and Discord — webhooks have no equivalent.

## 11.6 Telegram

`src/notifications/targets/telegram/`

- `TelegramSubscription` with pairing challenge, chat kind (Private/Group/
  Supergroup/Channel), min severity, lifecycle-event filter.
- `TelegramPairingChallenge { code_hash, created_at, expires_at, consumed_at }`.
- `PairingCode` with formatted dash-separated 8-char code from a 30-character
  alphabet (no 0/O/1/I/L collisions), HMAC-SHA256-hashed via
  `PairingCodeHash::from_code(secret, &code)`. `PairingCodeHash::eq` uses
  constant-time compare (`subtle::ConstantTimeEq`). Solid crypto hygiene.
- `TelegramCommand::{Start{code}, TestAlert, Status, Help, Unpair}` — the
  parsed user-facing bot vocabulary.
- `TelegramSender` — stub.
- `TelegramService { sender, config }` — held inside `Notifier`.

> **Divergence from prior spec:** Pairing is explicitly modeled — the prior
> spec listed pairing as an open question. The code has it: challenge
> creation, code hashing, expiry tracking, consumption.

## 11.7 Discord

`src/notifications/targets/discord/`

- `DiscordTarget` uses **webhook URL**, not bot token (with optional
  `thread_id`, `username_override`, `avatar_url_override`).
- `DiscordSubscription` with channel label, optional guild/channel IDs,
  min severity, lifecycle-event filter.
- `DiscordSeverityPalette` with hex colors per severity (info `#3498DB`,
  warning `#F39C12`, critical `#E74C3C`, resolved `#2ECC71`).
- `DiscordPayload` with full embed shape (title, description, color,
  timestamp, fields, allowed_mentions defaulting to `none()`).
- Render produces a single embed with optional "Affected" and "Diagnostic"
  fields.
- `DiscordSender` — stub.

## 11.8 Webhook

`WebhookTarget { url: SecretString, method: WebhookMethod::Post, headers }`.
Renders JSON body with incident_id/kind/severity/title/summary/affected/
diagnostic/occurred_at. Sender — stub.

## 11.9 Trait status

`ErasedSink` trait (`src/notifications/traits.rs`) exists but is **not used
by the orchestrator**. The orchestrator dispatches via concrete `match`
over `NotificationTarget`. The trait looks legacy or aspirational.

## 11.10 Persistence

**In code today:** none.

**Designed in ADR-P3:** durable notification attempts log with retry
queue. Every `Notifier::dispatch` call inserts a `Pending` row, then
updates it to a terminal status with the `DeliveryReceipt`. Transient
failures with retries remaining set `next_retry_at`; the consumer
task's retry-ticker (`select!` arm, 10s default tick) picks them up
and produces a new attempt row with `attempt_number + 1` and
`parent_attempt_id` linking back.

See § 13.4 for the `NotificationAttemptRepository` trait surface and
§ 13.bis (new) for the full schema.

Subscriptions remain `Serialize + Deserialize` types only — Telegram
pairing and Discord webhook configuration persistence is out of V0
scope.

### `Notifier::dispatch` signature change (ADR-P3 §P3.10)

```rust
// V0
pub async fn dispatch(
    &self,
    event: &IncidentLifecycleEvent,
    message: &NotificationMessage,
    attempts_repo: &dyn NotificationAttemptRepository,
    now: DateTime<Utc>,
) -> Vec<NotificationAttempt>;
```

The return type is now `Vec<NotificationAttempt>` (not just receipts)
so the caller can inspect retry state without a follow-up repository
read. Each attempt's `status` reflects the post-dispatch terminal
value; `next_retry_at` is populated for transient failures still in
retry budget.

## 11.11 Suppression filtering (designed; V0.1 — not in V0)

Per ADR-L5 §L5.4, the notifier consults `SuppressionRepository` before
dispatching to any sink. Sketch:

```rust
impl Notifier {
    pub async fn dispatch(&self, event: &IncidentLifecycleEvent,
                          message: &NotificationMessage)
        -> Vec<(NotificationRuleId, DeliveryReceipt)>
    {
        let fp = event.incident().fingerprint.clone();
        if let Some(rule_id) = self.suppression
            .matches(&fp, Utc::now())
            .await
            .unwrap_or(None)
        {
            // synthesize a Suppressed receipt for every notification rule that
            // would have matched, so the audit trail is complete
            return self.rules.iter()
                .filter(|r| r.matches(event))
                .map(|r| (r.id.clone(), suppressed_receipt(rule_id.clone())))
                .collect();
        }
        // existing dispatch logic — fan out to telegram/discord/webhook
    }
}
```

The `SuppressionRepository` is owned by the runtime and passed into
`Notifier::new` alongside the existing telegram/discord/webhook
services. In V0, neither the repository nor the suppression-check path
exists in the orchestrator; both are added in V0.1.

---

# 12. Runtime Pipeline

**Not implemented yet.** `src/main.rs:1-12` is still
`println!("hello world")`.

After ADRs L1–L5, R1–R3, C1–C3, and **S1–S3**, the entire runtime is
designed. What's missing is the code.

## 12.1 Architecture — per-collector tasks + central consumer

Per ADR-S1, collectors run as their own tokio tasks; observations flow
through a bounded `mpsc::channel` to a single consumer task that owns
the pipeline:

```text
[Polling collector tasks]                ─┐
  tokio::time::interval driven             │
  send ObservationBatch on each tick       │
                                           │   tokio::mpsc::channel
[Subscription collector tasks (V0.1+)]   ─┤   <ObservationBatch>
  long-lived stream consumers              │   bounded (default 1024)
  send batches as data arrives             │
                                           │
[Consumer task] (sole writer below) ───────┘
  loop:
    batch = rx.recv().await
    observation_store.append(batch.observations).await
    for obs in batch.observations:
        read_models.apply(obs)?           ← &mut self, no lock (single writer)
    subject = entity_ref_from(batch.collector.target)
    ctx = DiagnosticContext { now, subject, &read_models … }
    for rule in rules:
        drafts: Vec<UnvalidatedIncidentSignalDraft> = rule.evaluate(ctx)?  ← ADR-D1
    for draft in drafts:
        events = engine.handle(             ← &mut self, no lock (single writer)
            IncidentCommand::RecordSignal(draft), now)?    ← ADR-D4: Vec<IncidentEvent>
        for event in events:
            match event {
                SignalRecorded(obs)  → observation_store.append(&obs).await;
                                       read_models.apply(&obs)?
                IncidentTouched(inc) → incident_repo.save(&inc).await  ← write-through
                Lifecycle(ev)        → notifier.dispatch(&ev, &compose(&ev),
                                                          &attempts_repo, now).await
                                       ← inserts Pending row + UPDATE on completion
                                       ← (V0.1+) consults SuppressionRepository
                DraftRejected{…}     → tracing::warn!(…)
                DraftBelowConfidenceFloor{…} → tracing::debug!(…)
            }
        // (V1.0+ cloud sync) the same events are pushed to the cloud
        // control plane — no derivation step needed (ADR-D4).

  on retry_ticker.tick():               ← ADR-P3 §P3.7, default 10s
    retryable = attempts_repo.list_retryable(Utc::now(), 32).await
    for old_attempt in retryable:
      incident = incident_repo.get(old_attempt.incident_id).await
      event    = reconstruct_event(old_attempt.lifecycle_kind, incident)
      message  = compose_notification_message(&event)
      notifier.retry_one(old_attempt, &event, &message, &attempts_repo, now).await
        ← inserts new row with attempt_number+1, parent_attempt_id = old.id
```

The **single-consumer** property is architecturally load-bearing:
because only the consumer task ever calls `&mut self` on the read-model
store or the incident engine, no locking is needed. The channel is the
serialization primitive.

## 12.2 Components

| Component | Module | Source ADR |
|---|---|---|
| Per-collector task supervisor | `src/runtime/supervisor.rs` | ADR-S1 + ADR-S3 §S3.4 |
| Pipeline consumer | `src/runtime/consumer.rs` | ADR-S1 + ADR-S3 §S3.8 |
| Rule registry | `src/runtime/rules.rs` | ADR-S3 §S3.5 |
| Collector bootstrap | `src/runtime/bootstrap.rs` | ADR-C3 §C3.2 |
| Runtime config | `src/runtime/config.rs` | ADR-S3 §S3.2 |
| Top-level orchestrator | `src/runtime/mod.rs` (`pub fn run(deps)`) | ADR-S1 |

## 12.3 Backpressure, shutdown, supervision

- **Backpressure** (ADR-S3 §S3.2): bounded `mpsc::channel(1024)` —
  collectors block on `send` when the consumer is overloaded. Slow
  consumer is visible (collectors stall) rather than invisible (OOM).
- **Shutdown** (ADR-S3 §S3.3): `tokio::sync::broadcast::Sender<()>`
  signal triggered by SIGINT or SIGTERM. Collector tasks observe the
  signal and exit their loops; the channel closes when the last sender
  drops; the consumer drains remaining batches; everything wraps inside
  a 30s `tokio::time::timeout`.
- **Collector supervision** (ADR-S3 §S3.4): each collector task is
  spawned with a `JoinHandle`; the supervisor respawns on unexpected
  exit with exponential backoff (10s → 30s → 60s → 300s cap, resetting
  to 10s after a 5-minute clean run). Polling collectors are
  contract-bound not to panic (ADR-C2); a panic logs as a bug but
  doesn't crash the sidecar.

## 12.4 `main.rs` — the bootstrap

Per ADR-S3 §S3.7, `main.rs` becomes a thin bootstrap:

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let config = Config::load_from_args_and_env()?;            // (config ADR — TBD)
    let node_registry = NodeRegistry::from_config(&config)?;
    let kinds = KindRegistry::load(config.kinds_config.as_deref())?;
    let sidecar_id = SidecarId(uuid::Uuid::now_v7());

    let http = reqwest::Client::builder().timeout(Duration::from_secs(30)).build()?;

    let observation_store: Arc<dyn ObservationStore>     = Arc::new(/* storage ADR — TBD */);
    let incident_repo:     Arc<dyn IncidentRepository>   = Arc::new(/* storage ADR — TBD */);

    let read_models = ReadModelStore::new(ReadModelStoreConfig::default());
    let open_incidents = incident_repo.load_open().await?;
    let engine = IncidentEngine::new(kinds, sidecar_id, open_incidents);

    let notifier = Notifier::new(
        config.notification_rules.clone(),
        WebhookSender::new(http.clone()),
        config.telegram.as_ref().map(|c| TelegramService::new(c.clone())),
        config.discord.as_ref().map(|c| DiscordService::new(c.clone())),
    );

    let polling = bootstrap::build_polling_collectors(&config.collectors, &node_registry, &http)?;
    let subscription = bootstrap::build_subscription_collectors(&config.collectors, &node_registry, &http)?;

    runtime::run(RuntimeDeps {
        sidecar_id,
        polling_collectors: polling,
        subscription_collectors: subscription,
        rules: rules::all(),
        read_models,
        engine,
        notifier,
        observation_store,
        incident_repo,
        config: config.runtime,
    }).await
}
```

The `RuntimeDeps` struct keeps the function signature manageable.
Storage trait impls and the broader `Config` shape land in the next
ADR cluster.

## 12.5 What still needs an ADR cluster

- **Observation store backend.** JSONL append for V0, SQLite later?
  How indexed? Retention policy?
- **Incident repository backend.** Same question.
- **`Config` shape** (`bithound.toml` schema, env-var overrides,
  secrets handling, CLI flags).
- **Tracing / logging setup** (format, levels, where).
- **Migrations** for V0.1+ when the schema changes.

> **Divergence from prior spec (historical):** Sections 12.1–12.3 of
> the prior spec showed pseudocode and listed components
> (`CollectorScheduler`, `ObservationStore`, `ProjectorRegistry`,
> `ProjectionStore`, `DiagnosticEngine`, `IncidentService`,
> `NotificationRouter`, `NotificationSinkRegistry`). The runtime is now
> designed under different names that map to the actual decomposition:
> `runtime::supervisor` + `runtime::consumer` replace the scheduler
> abstraction; `ReadModelStore` replaces the projector registry;
> `IncidentEngine` replaces `IncidentService`; `Notifier` is the
> notification router. The names converge on the channel-based
> architecture in ADR-S1.

---

# 13. Storage Model

**Not implemented yet in code.** Designed in ADR-P1 + ADR-P2.

## 13.1 Backend

**SQLite via `sqlx`**, one database file (`bithound.db`), **four**
tables — `observations`, `incidents`, `suppression_rules`,
`notification_attempts` (ADR-P3). Hybrid schema: indexed columns for
hot fields + JSON column for the full domain object.

`Cargo.toml` will gain:
```toml
sqlx = { version = "0.8", default-features = false,
         features = ["runtime-tokio", "tls-rustls", "sqlite",
                     "chrono", "uuid", "migrate"] }
```

Macros disabled — runtime-checked queries (`sqlx::query`,
`sqlx::query_as`) rather than compile-time `query!` macros, to avoid
the build-time DB requirement that would impede contributions.

## 13.2 Schema (V0 initial migration)

`migrations/0001_initial.sql` is embedded in the binary and run on
first start via `sqlx::migrate!("./migrations")`. See ADR-P1 for the
full DDL.

Key design choices:
- **`STRICT` tables** (SQLite 3.37+) for type rigor.
- **UUIDv7 IDs stored as `BLOB`** (16 bytes) — sortable, indexed.
- **Timestamps as `INTEGER` unix nanos** — round-trips cleanly through
  `chrono::DateTime<Utc>`.
- **`payload_json` / `incident_json`** — full serde-typed domain
  object as JSON, so new variants don't force schema migrations.

## 13.3 Pool & durability settings

```rust
SqlitePoolOptions::new()
    .max_connections(8)
    .connect(&format!("sqlite://{}?mode=rwc", path.display())).await?
PRAGMA journal_mode = WAL
PRAGMA synchronous  = NORMAL
```

- **WAL mode** — concurrent readers alongside the single writer
  (per ADR-S1, the consumer task is the sole writer).
- **`synchronous = NORMAL`** — fdatasync on commit (not per page);
  3-10× faster than FULL; small durability window. Acceptable because
  observations are recoverable from collectors on the next tick.

## 13.4 Trait surface

`ObservationStore` is **new in ADR-P2** (it didn't exist before this
cluster):

```rust
#[async_trait]
pub trait ObservationStore: Send + Sync {
    async fn append_many(&self, batch: &[Observation]) -> Result<(), StoreError>;
    async fn iter_since(
        &self, since: DateTime<Utc>,
    ) -> Result<BoxStream<'_, Result<Observation, StoreError>>, StoreError>;
}

pub enum StoreError {
    Io(std::io::Error),
    Database(sqlx::Error),
    Serialization(serde_json::Error),
    Corruption(String),
    NotInitialized,
}
```

`IncidentRepository` (ADR-L4) and `SuppressionRepository` (ADR-L5) use
the same SQLite backend; the trait signatures are unchanged from
their original ADRs.

`NotificationAttemptRepository` (ADR-P3) lives in
`src/notifications/repository.rs`:

```rust
#[async_trait]
pub trait NotificationAttemptRepository: Send + Sync {
    async fn insert_pending(&self, attempt: &NotificationAttempt) -> Result<(), RepoError>;
    async fn complete(
        &self,
        id: &NotificationAttemptId,
        receipt: DeliveryReceipt,
        next_retry_at: Option<DateTime<Utc>>,
    ) -> Result<(), RepoError>;
    async fn list_retryable(
        &self,
        now: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<NotificationAttempt>, RepoError>;
    async fn list_for_incident(
        &self,
        incident_id: &IncidentId,
    ) -> Result<Vec<NotificationAttempt>, RepoError>;
}
```

The `notification_attempts` table uses **per-row immutability**: each
attempt INSERTs as `Pending`, UPDATEs to exactly one terminal status
(`Succeeded`, `FailedTransient`, `FailedPermanent`, `Suppressed`), and
never moves between terminals. Retries produce *new rows* with
`attempt_number + 1` and `parent_attempt_id` linking to the prior
attempt. The retry chain is walked via the `parent_attempt_id` pointer.

## 13.5 Concurrency

**No manual interior mutability** anywhere. `SqlitePool` is `Clone`
and internally `Arc`-shared. Repository methods take `&self`, hold a
`SqlitePool`, and call `sqlx::query(…)`.

This replaces the `tokio::sync::Mutex<File>` pattern that JSONL would
have required.

## 13.6 Retention

A `retention::run(pool, config, shutdown_rx)` background task runs in
its own tokio task alongside the supervisor. Periodically deletes
rows older than configured ages and runs `VACUUM`. Defaults:

| Table | Default max age | Knob |
|---|---|---|
| `observations` | 30 days | `observations_max_age_days` |
| `incidents` (resolved only) | 365 days | `incidents_max_age_days` |
| `suppression_rules` (past `until`) | 90 days | `suppressions_grace_days` |
| `notification_attempts` (non-Pending) | 30 days | `attempts_max_age_days` |

Vacuum runs every 24h by default (`vacuum_interval_hours`). All can be
overridden in `bithound.toml`. `0` disables retention for that table.

The `notification_attempts` sweep skips rows still in `Pending` state
so an in-flight attempt is never deleted from under the dispatcher.

## 13.7 Cloud migration path (V0.2+)

sqlx abstracts both SQLite and Postgres. The repository
implementations target the SQLite flavor; lifting to Postgres for
cloud sync is largely mechanical. The schema is portable apart from
the `STRICT` keyword (Postgres has stricter types natively, so the
guarantee is preserved).

## 13.8 In-memory test impls

`MemoryObservationStore` and `MemoryIncidentRepository` (under
`src/storage/memory/`) give unit/integration tests a zero-I/O backend.
Free correctness check that the consumer pipeline works against any
trait impl, not just sqlx.

> **Divergence from prior spec (historical):** The original spec
> proposed unspecified schemas. ADR-P1 settles on SQLite + sqlx +
> hybrid columns. Cloud-readiness was the deciding factor.

---

# 13.bis Config

Designed in ADR-X1. Lives in `src/config/`.

## 13.bis.1 — Single `bithound.toml`

One file. Default path resolution: `--config <path>` → `./bithound.toml`
→ `/etc/bithound/bithound.toml` → hard error.

## 13.bis.2 — Schema (V0)

```toml
[sidecar]
id_file = "/var/lib/bithound/sidecar_id"
log_level = "info"

[storage]
db_path = "/var/lib/bithound/bithound.db"

[storage.retention]
observations_max_age_days = 30
incidents_max_age_days    = 365
suppressions_grace_days   = 90
attempts_max_age_days     = 30      # ADR-P3 §P3.11
vacuum_interval_hours     = 24

[runtime]
channel_capacity = 1024
shutdown_deadline_seconds = 30
notification_max_retries  = 3       # ADR-P3 §P3.6
notification_retry_tick_seconds = 10  # ADR-P3 §P3.7

[incidents]
kinds_config_path = "/etc/bithound/custom_kinds.toml"   # optional, ADR-L1

[[bitcoin_nodes]]
id = "btc-alice"
rpc_url = "http://127.0.0.1:8332"
zmq_endpoint = "tcp://127.0.0.1:28332"

[bitcoin_nodes.auth]
type = "user_pass"
user = "bithound"
password_env = "BITHOUND_BITCOIN_ALICE_PASSWORD"

[[collectors]]
id = "btc-alice-rpc"
target = { type = "bitcoin_node", id = "btc-alice" }
integration = { type = "bitcoin_core_rpc", interval_seconds = 10 }
instance_label = "alice"

[notifications.telegram]
bot_token_env = "BITHOUND_TELEGRAM_BOT_TOKEN"
parse_mode = "html"

[[notification_rules]]
id = "00000000-0000-7000-8000-000000000001"
name = "critical-to-telegram"
enabled = true
min_severity = "critical"
event_kinds = []

[notification_rules.target]
type = "telegram"
chat_id = -1001234567890
```

## 13.bis.3 — Secrets

**Env-var references only** for V0. Field suffix `_env` names the env
var. Inline values in `*_password`, `*_token`, `*_secret` fields are a
hard parse error. `SecretString` wraps loaded values.

Container-orchestration friendly out of the box; file-ref support
(`*_file`) can be added later if real use cases emerge.

## 13.bis.4 — CLI

```rust
struct Cli {
    /// Path to bithound.toml.
    #[arg(long, short)] config: Option<PathBuf>,
    /// Print merged config (secrets redacted) and exit.
    #[arg(long)] check_config: bool,
    /// Print version and exit.
    #[arg(long)] version: bool,
}
```

`clap` (derive feature) becomes a new dependency.

## 13.bis.5 — Env-var overrides for non-secret keys

`BITHOUND_<SECTION>__<KEY>=value` (double underscore separator).
Applied after TOML parse, before secrets resolution. Used for
testing and one-off runs; production typically uses the TOML file.

## 13.bis.6 — Validation

Fail-loud at startup. `Config::load_from_args_and_env()` validates
TOML shape, cross-references (collectors reference existing nodes),
env-var presence (without reading values yet), and writable storage
paths. Any failure → `ConfigError`, exit code 78 (EX_CONFIG), one-line
message.

## 13.bis.7 — SidecarId persistence

`sidecar.id_file` is read on startup; if absent or unparseable, a
fresh `Uuid::now_v7()` is generated and written. Keeps sidecar
identity stable across restarts.

---

# 14. Commands, Events, and Facts

The current observed split:

| Domain | Style |
|---|---|
| Collectors | Service-y. `CollectionContext` → `ObservationBatch`. |
| Observations | **Facts.** Immutable, append-only by intent. |
| Read Models | Query traits, no apply path in the trait. |
| Diagnostics | Pure-ish. `DiagnosticContext` → `Vec<IncidentSignalDraft>`. |
| Incidents | **Data + lifecycle event tag.** No commands, no service. |
| Notifications | `NotificationRule` + `dispatch` method. No command type. |

> **Divergence from prior spec:** Sections 14.1–14.5 of the prior spec
> proposed `IncidentCommand`, `NotificationCommand`, `ObservationEvent`
> types. None of those exist. The system is not event-sourced.

What exists is closer to **CQRS-lite without the C**: there are read
models and there are immutable facts, but no formal command channel.
Writes happen by directly mutating in-memory state or appending facts
(once a store exists).

---

# 15. Module Layout

Sections (a) and (b) below are kept separate because the gap between
**code today** and **post-ADR design** is the most useful reference
during implementation.

## 15.a Actual layout (code today, 2026-05-17)

```text
bithound/
├── Cargo.toml                                  # bithound 0.1.0, single binary
├── README.md
├── docs/
│   └── INCIDENT_CATALOG.md                     # 17 candidate diagnostic rules
└── src/
    ├── main.rs                                 # "hello world" stub
    ├── rpc.rs                                  # empty (whitespace)
    │
    ├── shared/
    │   ├── mod.rs
    │   └── types.rs                            # IDs, EntityRef, EvidenceRef
    │
    ├── collectors/
    │   ├── mod.rs
    │   ├── types.rs                            # CollectorDescriptor, IntegrationKind, …
    │   ├── traits.rs                           # EMPTY
    │   ├── error.rs                            # EMPTY (errors live in types.rs)
    │   └── registry.rs                         # NodeRegistry, BitcoinRpcAuth, …
    │
    ├── observations/
    │   ├── mod.rs
    │   └── types.rs                            # Observation, ObservationBatch, ProbeWindow, ProbeResult, Attributes, …
    │       └─ types/                           (subdir as `mod types/`)
    │           ├── source.rs                   # ObservationSource, ObservationOrigin
    │           ├── metric.rs                   # MetricObservation + units + histogram/summary
    │           ├── state.rs                    # StateObservation enum, 8 typed variants
    │           ├── event.rs                    # EventObservation, EventSeverity
    │           ├── health.rs                   # HealthCheckObservation, HeartbeatObservation
    │           ├── capability.rs               # CapabilityObservation
    │           ├── inventory.rs                # InventoryObservation
    │           ├── transition.rs               # TransitionObservation, StateAtom
    │           ├── diagnosis.rs                # DiagnosisObservation (not in payload enum)
    │           └── incident_signal.rs          # IncidentSignalObservation (not in payload enum)
    │
    ├── read_models/
    │   ├── mod.rs
    │   ├── types.rs                            # Projected<T>
    │   └── traits/
    │       ├── state.rs                        # StateReadModel
    │       ├── metric.rs                       # MetricReadModel
    │       ├── health.rs                       # HealthReadModel
    │       ├── capability.rs                   # CapabilityReadModel
    │       ├── heartbeat.rs                    # HeartbeatReadModel
    │       └── incident_signal.rs              # IncidentSignalReadModel
    │
    ├── diagnostics/
    │   ├── mod.rs                              # `mod traits; mod types;` (not pub)
    │   ├── traits.rs                           # DiagnosticRule
    │   └── types.rs                            # DiagnosticContext, IncidentSignalDraft
    │
    ├── incidents/
    │   ├── mod.rs
    │   └── types.rs                            # Incident, IncidentKind, severity, status, lifecycle event
    │
    └── notifications/
        ├── mod.rs
        ├── types.rs                            # NotificationRule, NotificationMessage, DeliveryOutcome taxonomy
        ├── traits.rs                           # ErasedSink (legacy, unused)
        ├── orchestrator.rs                     # Notifier::dispatch
        └── targets/
            ├── mod.rs
            ├── webhook/                        # WebhookTarget/Payload/Sender (sender = stub)
            ├── discord/                        # DiscordTarget/Subscription/Payload/Sender/Service (sender = stub)
            └── telegram/                       # TelegramTarget/Subscription/Pairing/Sender/Service (sender = stub)
                                                # PairingCode + HMAC-SHA256 PairingCodeHash with constant-time eq
```

## 15.b Target layout after V0 ADRs (post-S3, post-P/X)

```text
bithound/
├── Cargo.toml                                  # + sqlx, clap deps
├── README.md
├── config/
│   └── default_kinds.toml                      # embedded via include_str! (ADR-L1 §6)
├── migrations/                                 # ADR-P1
│   └── 0001_initial.sql                        # observations, incidents, suppression_rules
├── docs/
│   └── INCIDENT_CATALOG.md
└── src/
    ├── main.rs                                 # ADR-S3 §S3.7 — thin bootstrap
    │
    ├── shared/
    │   ├── mod.rs
    │   └── types.rs                            # + EntitySubjectKind (ADR-L1 §3), ActorId (ADR-L5 §L5.5)
    │
    ├── collectors/                             # ADR-C1, C3
    │   ├── mod.rs
    │   ├── traits.rs                           # PollingCollector, SubscriptionCollector, BatchSink
    │   ├── types.rs                            # + sidecar_id on CollectionContext (ADR-C3 §C3.1)
    │   ├── registry.rs                         # NodeRegistry (existing)
    │   ├── bitcoin_core/
    │   │   ├── mod.rs
    │   │   ├── rpc.rs                          # BitcoinCoreRpcCollector (V0)
    │   │   ├── rpc_client.rs                   # BitcoinRpcClient + RpcError (V0)
    │   │   └── zmq.rs                          # BitcoinCoreZmqCollector (V0.1+)
    │   ├── lnd/                                # V0.1+
    │   └── host/                               # V0.1+
    │
    ├── observations/
    │   ├── mod.rs
    │   ├── types.rs                            # + Diagnosis, IncidentSignal in ObservationPayload (ADR-R2)
    │   ├── events.rs                           # ObservationEvent (ADR-D4)
    │   └── types/
    │       ├── (existing variants)
    │       └── state/
    │           └── well_known.rs               # canonical StateName constants (ADR-R1 §R1.2)
    │
    ├── read_models/                            # ADR-R1
    │   ├── mod.rs
    │   ├── types.rs                            # Projected<T>
    │   ├── store.rs                            # ReadModelStore (assembler)
    │   ├── traits/
    │   │   ├── state.rs                        # StateReadModel (rewritten — generic)
    │   │   ├── state_ext.rs                    # StateReadModelExt (typed helpers, auto-impl)
    │   │   ├── metric.rs                       # MetricReadModel
    │   │   ├── health.rs                       # HealthReadModel
    │   │   ├── capability.rs                   # CapabilityReadModel
    │   │   ├── heartbeat.rs                    # HeartbeatReadModel
    │   │   └── incident_signal.rs              # IncidentSignalReadModel
    │   └── projections/                        # ADR-R1 §R1.4 — one per observation type
    │       ├── mod.rs                          # Projection trait + ProjectionError
    │       ├── state.rs
    │       ├── metric.rs
    │       ├── health.rs
    │       ├── capability.rs
    │       ├── heartbeat.rs
    │       └── incident_signal.rs
    │
    ├── diagnostics/
    │   ├── mod.rs                              # pub mod (ADR-001 §3)
    │   ├── traits.rs                           # DiagnosticRule (emits Vec<UnvalidatedIncidentSignalDraft>)
    │   ├── types.rs                            # DiagnosticContext + signals field (ADR-001 §4)
    │   │                                       # UnvalidatedIncidentSignalDraft (ADR-D1, L1)
    │   ├── events.rs                           # DiagnosticEvent (ADR-D4)
    │   └── rules/                              # one module per rule, lands incrementally
    │       ├── bitcoin/
    │       │   ├── tip_lag.rs                  # A1
    │       │   └── peer_starvation.rs          # A3
    │       └── host/
    │           └── disk_exhaustion.rs          # X1
    │
    ├── incidents/                              # ADR-L1, L3, L4, L5
    │   ├── mod.rs
    │   ├── types.rs                            # Incident (+ fingerprint, Vec<ObservationId>, Suppressed)
    │   ├── engine.rs                           # IncidentEngine, IncidentCommand, IncidentEvent (ADR-L4, D3, D4)
    │   ├── events.rs                           # IncidentEvent enum (ADR-D4)
    │   ├── repository.rs                       # IncidentRepository trait (ADR-L4)
    │   ├── kinds.rs                            # IncidentKindSpec, KindRegistry (ADR-L1)
    │   ├── well_known.rs                       # canonical IncidentKind &'static str (ADR-L1 §5)
    │   └── suppression.rs                      # SuppressionRule, SuppressionRepository (ADR-L5; V0.1)
    │
    ├── notifications/
    │   ├── mod.rs
    │   ├── types.rs                            # + DeliveryOutcome::Suppressed (ADR-L5 §L5.4)
    │   │                                       # + revised NotificationAttempt (ADR-P3 §P3.4)
    │   ├── orchestrator.rs                     # Notifier — suppression-aware (ADR-L5)
    │   │                                       # + dispatch signature change (ADR-P3 §P3.10)
    │   ├── repository.rs                       # NotificationAttemptRepository (ADR-P3)
    │   └── targets/                            # unchanged from code today; senders to be implemented
    │
    ├── runtime/                                # ADR-S1, S3
    │   ├── mod.rs                              # pub fn run(deps) -> Result<…>; RuntimeDeps
    │   ├── supervisor.rs                       # collector supervision + shutdown
    │   ├── consumer.rs                         # pipeline consumer task
    │   ├── rules.rs                            # rules::all()
    │   ├── bootstrap.rs                        # build_polling_collectors / build_subscription_collectors
    │   └── config.rs                           # RuntimeConfig
    │
    ├── storage/                                # ADR-P1, P2
    │   ├── mod.rs
    │   ├── traits.rs                           # ObservationStore + StoreError (new)
    │   ├── retention.rs                        # background retention task
    │   ├── sqlite/
    │   │   ├── mod.rs                          # open_pool helper
    │   │   ├── observation_store.rs            # SqliteObservationStore
    │   │   ├── incident_repository.rs          # SqliteIncidentRepository
    │   │   ├── notification_attempt_repository.rs  # SqliteNotificationAttemptRepository (ADR-P3)
    │   │   └── suppression_repository.rs       # SqliteSuppressionRepository (V0.1)
    │   └── memory/                             # in-memory test impls
    │       ├── observation_store.rs
    │       ├── incident_repository.rs
    │       └── notification_attempt_repository.rs  # MemoryNotificationAttemptRepository (ADR-P3)
    │
    ├── config/                                 # ADR-X1
    │   ├── mod.rs                              # Config::load_from_args_and_env, ConfigError
    │   ├── sidecar.rs                          # SidecarConfig
    │   ├── storage.rs                          # StorageConfig, RetentionConfig
    │   ├── runtime.rs                          # RuntimeConfig
    │   ├── targets.rs                          # BitcoinNodeConfig + AuthConfig (V0.1: Lnd/Host)
    │   ├── collectors.rs                       # CollectorDescriptorConfig
    │   ├── notifications.rs                    # NotificationRulesConfig + per-sink configs
    │   ├── secrets.rs                          # *_env resolution
    │   └── cli.rs                              # Cli (clap derive)
    │
    ├── notifications/events.rs                 # NotificationEvent (ADR-D4)
    ├── read_models/events.rs                   # ReadModelEvent (ADR-D4)
    ├── shared/parse.rs                         # parse_dotted_name + ParseDottedNameError (ADR-D2)
    └── domain_events.rs                        # top-level DomainEvent envelope (ADR-D4)
```

---

# 16. V0 Product Boundary

The prior spec's V0 scope still applies. Concretely, **what is left to do
for a runnable V0**:

1. Write the `Collector` trait into the empty `src/collectors/traits.rs`.
2. Implement at least one collector (`BitcoinCoreRpcCollector` is the
   highest-leverage starting point — the `BitcoinBlockchainState` /
   `BitcoinMempoolState` / `BitcoinNetworkState` / `BitcoinPeerSummaryState`
   typed states are already designed for the matching getblockchaininfo /
   getmempoolinfo / getnetworkinfo / getpeerinfo responses).
3. Build a concrete in-memory store that implements all six observation-facing
   read-model traits and exposes an `apply(&Observation)` mutation path.
4. Write a few of the `INCIDENT_CATALOG.md` rules as `DiagnosticRule` impls.
   Recommended first cuts: A3 (low peer count), A1 (tip lag — needs
   `MetricReadModel::unchanged_for`), X1 (disk exhaustion).
5. Build an incident engine: take `IncidentSignalDraft`s, decide what they
   mean for current `Incident`s, emit `IncidentLifecycleEvent`s. This is
   where fingerprinting must be designed.
6. Implement the three notification senders (telegram, discord, webhook)
   replacing the BadRequest stubs.
7. Write the runtime loop in `main.rs`: schedule collectors per
   `IntegrationKind::interval`, run probes, append observations, update
   read models, run diagnostics, run incident engine, dispatch.

Out of scope (matching the prior spec's V0 exclusions): cloud sync,
multi-tenancy, IAM, billing, AI diagnosis, dynamic plugin system.

---

# 17. Suggested Implementation Order

Adjusted for current state:

1. `Collector` trait (file exists, just empty).
2. `BitcoinCoreRpcCollector` against an existing regtest node.
3. In-memory store implementing the read-model traits (this unblocks
   diagnostics).
4. First two or three diagnostic rules from `INCIDENT_CATALOG.md`.
5. Incident engine + fingerprinting decision.
6. Replace one notification sender stub end-to-end (Telegram is most
   complete on the metadata side).
7. Wire `main.rs`: scheduler → collect → apply → diagnose → engine → notify.
8. Persistence: first cut can be JSONL append-only files for observations
   and a JSON snapshot for active incidents, before introducing SQLite.

---

# 18. Agent Reconciliation Checklist

## 18.1 Type inventory

See § 21 below.

## 18.2 Boundary check

- Collectors emit observations only — *will*, once the trait & implementors
  exist. The types preclude any other output.
- Diagnostics emit `IncidentSignalDraft`s only — confirmed by `DiagnosticRule`.
- The incident layer owns lifecycle transitions in *type* (`IncidentLifecycleEvent`),
  but **no service is written**, so there's no behavioral check.
- Notifications consume incident lifecycle events only — confirmed by
  `Notifier::dispatch`.
- Read models are projections, not authorities — confirmed by the
  query-only trait shape.

## 18.3 Naming

- Names are *mostly* consistent, with three exceptions:
  - `IncidentStatus::Supressed` — typo of `Suppressed`.
  - `Incident.signal_observation_ids: Option<ObservationId>` — plural name,
    singular type.
  - `DiagnosisName`/`SignalName` newtypes are defined but the file
    `diagnostics/mod.rs` doesn't `pub use` them, which means external code
    cannot construct rules yet.
- ID newtypes are used pervasively. No bare `Uuid` or `String` leaks at
  domain boundaries — good.

## 18.4 Runtime

- Collectors are scheduled by … nothing. No scheduler exists. Intervals
  live on `IntegrationKind` and wait for a consumer.
- Observations are persisted by … nothing. No store.
- Projections are updated by … nothing. The read-model traits are query-only.
- Diagnostics are triggered by … nothing.
- Incidents are persisted by … nothing.
- Notifications dispatch via `Notifier`, but no caller exists.

## 18.5 Persistence

- Database: none.
- Migrations: none.
- Append-only observations: not built; types support it.
- Durable incidents: not built; types support it (`evidence_summary` field
  shows future intent).
- Notification delivery records: types exist (`NotificationAttempt`,
  `DeliveryReceipt`) but no storage.

---

# 19. Known Design Decisions (as of current code)

| Decision | What the code does |
|---|---|
| Subject identity | Per-entity newtypes unified via `EntityRef`; no `TargetId`. |
| Collector output | `ObservationBatch` with `ProbeWindow` and `ProbeResult::{Ok, Failed}`. |
| Failed probe must carry health | Yes — non-optional field on `ProbeResult::Failed`. |
| Observation payload shape | Eight strongly-typed variants in `ObservationPayload`. |
| State payload shape | Strongly typed enum per subsystem, not `StateName`+`StateValue`. |
| Attribute values | Bounded enum, not JSON. |
| Read model results | `Projected<T>` carries observation_id + observed_at. |
| Diagnostic rule output | `IncidentSignalDraft` with `Active/Cleared` status and `Low/Medium/High` confidence. |
| Incident dedup | **Not designed yet.** No fingerprint field. |
| Incident lifecycle | `Opened`, `Escalated{prev, new}`, `Resolved` — no generic `Updated`. |
| Notification matching | Severity floor + optional incident-kind allowlist; empty list = match all kinds. |
| Sink dispatch | Concrete enum match, not trait object. `ErasedSink` trait exists but is unused. |
| Stdout sink | Debug-only (`cfg(debug_assertions)`). |
| Webhook auth | URL + custom headers (both `SecretString`). |
| Discord delivery | Webhook URL (not bot token). |
| Telegram pairing | Implemented: 8-char alphanumeric code, HMAC-SHA256 hash, constant-time comparison, expiry. |
| Persistence | None. |
| Runtime | None. |

---

# 20. Open Design Questions (refreshed)

## 20.1 Observation typing

Resolved in favor of strong typing (option 1 / "strongly typed enums per
domain object"), at least for state. The hybrid the prior spec recommended
is not the chosen direction. Open *secondary* question: are
`DiagnosisObservation` and `IncidentSignalObservation` going to become
`ObservationPayload` variants, or live in a parallel "derived observation"
channel?

## 20.2 Runtime orchestration

Still unresolved. Recommendation unchanged: V0 runs diagnostics after each
collector cycle and after each ZMQ-driven event. Open: how subscription
collectors (`BitcoinCoreZmq`, `LndGrpcStream`) feed into the same loop —
likely a bounded channel that the diagnostic pump drains alongside polling.

## 20.3 Incident resolution

Diagnostic rules emit `IncidentSignalDraft` with explicit `SignalStatus::Cleared`.
That gives the incident engine a strong signal to resolve. Hysteresis is
not modeled in type — must be implemented per-rule.

## 20.4 Notification delivery guarantees

Types are ready for durability (`NotificationAttempt`, `DeliveryReceipt`,
`Transient`/`Permanent` split, `external_ref` for follow-ups). Storage
isn't. Recommendation: V0 best-effort + log; V0.1 add a delivery table.

## 20.5 Stable IDs

The code already uses stable, externally-derivable IDs for entities
(pubkeys, hostnames, chan_ids as strings) — this answers "use globally
stable IDs even before cloud exists" with yes. `SidecarId` and the various
internal UUIDs (observation/incident/batch) are UUIDv7, giving lexicographic
time-ordering for free.

## 20.6 New open questions surfaced by the code

Status after the ADR-L1–L5 round:

- **Fingerprinting.** **Resolved by ADR-L1**:
  `(EntityRef, IncidentKind, Option<String>)`, engine-computed at receipt,
  validated against `KindRegistry`.
- **Diagnostic context completeness.** **Resolved by ADR-001 §4**:
  `DiagnosticContext` gains `signals: &dyn IncidentSignalReadModel`.
- **`Incident.signal_observation_ids` shape.** **Resolved by ADR-001 §2**:
  `Vec<ObservationId>`.
- **Diagnostics module not exported.** **Resolved by ADR-001 §3**:
  `pub mod` on submodules.
- **Suppressed/`Supressed` typo.** **Resolved by ADR-001 §1**: renamed
  to `Suppressed`.
- **Suppression semantics.** **Resolved by ADR-L5** (notifier-side
  filtering; V0.1 ships).

**Still open** (next ADR cluster):

- **Read-model update path.** **Resolved by ADR-R1 + ADR-R3**: six
  per-type projections behind a thin `ReadModelStore`; `apply(&mut self,
  &Observation)` method on the concrete struct.
- **`StateReadModel` shape.** **Resolved by ADR-R1 §R1.1**: rewritten
  to be generic over `StateName` instead of per-variant.
- **Derived observations in `ObservationPayload`.** **Resolved by
  ADR-R2**: `IncidentSignal` and `Diagnosis` promoted to first-class
  payload variants.
- **Collector trait.** **Resolved by ADR-C1 + ADR-C2 + ADR-C3**: two
  traits (`PollingCollector`, `SubscriptionCollector`); polling returns
  `ObservationBatch` directly; `BitcoinCoreRpcCollector` is the V0
  concrete impl.
- **Scheduler shape.** **Resolved by ADR-S1**: per-collector tokio
  tasks + central consumer task with bounded `mpsc::channel`. Single
  consumer enforces the single-writer property for read models and
  the incident engine.
- **Rule evaluation trigger.** **Resolved by ADR-S2**: per-batch
  evaluation against the batch's subject; rules return `Ok(vec![])`
  if not applicable.
- **Observation store backend.** **Resolved by ADR-P1**: SQLite via
  sqlx; cloud-portable to Postgres.
- **Incident repository backend.** **Resolved by ADR-P1**: same.
- **Storage trait shapes.** **Resolved by ADR-P2**: `ObservationStore`
  trait added with `append_many` + `iter_since`. Retention via
  background task, not rotation.
- **`bithound.toml` config layout.** **Resolved by ADR-X1**: single
  file with full V0 schema.
- **Secrets handling.** **Resolved by ADR-X1**: env-var refs only
  (`*_env` field suffix), no inline secrets.
- **Acknowledged / manual-resolve commands.** **Resolved by ADR-D3**:
  full `IncidentCommand` vocabulary defined in V0; V0.2 handlers
  return `EngineError::NotYetImplemented`.
- **Suppression commands.** **Resolved by ADR-D3**: separate
  `SuppressionCommand` enum + `SuppressionService` trait.
- **Validation-state typing.** **Resolved by ADR-D1**: two distinct
  structs `UnvalidatedIncidentSignalDraft` and
  `ValidatedIncidentSignalDraft`; private inner fields on the
  validated form make `KindRegistry::validate` the only construction
  path.
- **Name-newtype validation.** **Resolved by ADR-D2**: shared
  `parse_dotted_name` + smart constructors for all ten dotted-namespace
  newtypes. Serde re-validates via `try_from = "String"`.
- **Workflow output shape.** **Resolved by ADR-D4** (supersedes ADR-L4
  §L4.2): engine returns `Vec<IncidentEvent>`; `HandleOutcome` removed.
  Per-context events modules + top-level `DomainEvent` envelope.
  Cloud-sync ready out of the box.
- **`ActorId` location.** **Resolved by ADR-D3**: promoted to
  `src/shared/types.rs`.
- **Maintenance-window TOML schema** (V0.1).
- **BitcoinRpcAuth cookie refresh strategy** (read-on-each vs cache;
  ADR-C3 §C3.8 deferred this).
- **Tracing / logging setup** (format, levels, destination).
- **File-ref secrets** (`*_file` in addition to `*_env`) — deferred.

---

# 21. Current Type Inventory

## 21.1 Modules / crates found

Single binary crate `bithound` 0.1.0. Top-level modules under `src/`:
`collectors`, `diagnostics`, `incidents`, `notifications`, `observations`,
`read_models`, `rpc` (empty), `shared`. No sub-crates.

## 21.2 Domain types found

### shared
**In code:** `CollectorId`, `IncidentId`, `ObservationId`,
`ObservationBatchId`, `SidecarId`, `EvidenceRef`, `EntityRef`, `HostId`,
`BitcoinNodeId`, `BitcoinPeerId`, `LndNodeId`, `LndPeerId`,
`LndChannelId`, `LndInvoiceId`.

**Designed (ADR-D2, ADR-D3, ADR-L1 §3):** `EntitySubjectKind`,
`ActorId`, `parse_dotted_name` + `ParseDottedNameError` (in
`src/shared/parse.rs`).

### observations
`Observation`, `ObservationContext`, `ObservationBatch`, `ProbeWindow`,
`ProbeWindowError`, `ProbeResult`, `Attributes`, `AttributeValue`,
`ObservationPayload`, `ObservationSource`, `ObservationOrigin`,
`MetricObservation`, `MetricName`, `MetricKind`, `MetricValue`,
`NumericValue`, `Unit`, `HistogramValue`, `HistogramBucket`,
`SummaryValue`, `Quantile`, `StateObservation` (enum with 8 variants),
`StateName`, `StateValue` (defined, currently unused),
`BitcoinBlockchainState`, `BitcoinMempoolState`, `BitcoinNetworkState`,
`BitcoinPeerSummaryState`, `LndNodeState`, `LndWalletState`,
`LndChannelSummaryState`, `HostState`, `EventObservation`, `EventName`,
`EventSeverity`, `HealthCheckObservation`, `HeartbeatObservation`,
`HeartbeatStatus`, `CollectorStatus`, `HealthTargetId`, `HealthStatus`,
`HealthError`, `CapabilityObservation`, `CapabilityName`, `CapabilityStatus`,
`InventoryObservation`, `InventoryName`, `InventoryValue`,
`TransitionObservation`, `TransitionName`, `StateAtom`,
`DiagnosisObservation`, `DiagnosisName`, `Confidence`,
`IncidentSignalObservation`, `SignalName`, `SignalSeverity`, `SignalStatus`.

### collectors
`CollectionRunId`, `CollectionContext`, `CollectorSetup`,
`CollectorDescriptor`, `CollectorRef`, `CollectorMode`, `IntegrationKind`,
`CollectorTarget`, `CollectionError`, `CollectionErrorKind`,
`NodeRegistry`, `BitcoinNodeConnection`, `BitcoinRpcAuth`,
`LndNodeConnection`, `HostConnection`.

### read_models
`Projected<T>`.

### diagnostics
`DiagnosticContext<'a>`, `IncidentSignalDraft`.

### incidents
`Incident`, `IncidentKind`, `IncidentSeverity`, `IncidentStatus`,
`IncidentNotificationEventKind`, `IncidentLifecycleEvent`.

### notifications
`NotificationId`, `NotificationRuleId`, `NotificationAttemptId`,
`NotificationTargetId`, `NotificationRuleName`, `NotificationRule`,
`NotificationMessage`, `NotificationAttempt`, `DeliveryReceipt`,
`NotificationSource`, `NotificationKind`, `NotificationDeliveryStatus`,
`NotificationTarget`, `DeliveryOutcome`, `TransientError`, `PermanentError`,
`ExternalMessageRef`, `Notifier`,
`WebhookTarget`, `WebhookMethod`, `WebhookHeader`, `WebhookPayload`,
`WebhookSender`,
`DiscordGuildId`, `DiscordChannelId`, `DiscordMessageId`, `DiscordThreadId`,
`DiscordSubscriptionId`, `DiscordTarget`, `DiscordSubscription`,
`DiscordSetup`, `DiscordNotificationConfig`, `DiscordSeverityPalette`,
`DiscordPayload`, `DiscordEmbed`, `DiscordEmbedField`, `DiscordEmbedFooter`,
`DiscordEmbedAuthor`, `DiscordAllowedMentions`, `DiscordMentionType`,
`DiscordSender`, `DiscordService`,
`TelegramChatId`, `TelegramUserId`, `TelegramSubscriptionId`,
`TelegramPairingChallengeId`, `TelegramSubscription`, `TelegramTarget`,
`TelegramPairingChallenge`, `TelegramChatKind`, `TelegramParseMode`,
`TelegramCommand`, `PairingCode`, `PairingCodeHash`, `TelegramSetup`,
`TelegramNotificationConfig`, `TelegramPayload`, `TelegramReplyMarkup`,
`TelegramInlineButton`, `TelegramSender`, `TelegramService`.

## 21.3 Traits found

**In code today:**

| Trait | Module | Used? |
|---|---|---|
| `StateReadModel` | read_models | By `DiagnosticContext`. **Rewritten in ADR-R1 §R1.1** — per-variant methods removed, replaced with generic `latest_state` / `states_for`. |
| `MetricReadModel` | read_models | By `DiagnosticContext`. |
| `HealthReadModel` | read_models | By `DiagnosticContext`. |
| `CapabilityReadModel` | read_models | By `DiagnosticContext`. |
| `HeartbeatReadModel` | read_models | By `DiagnosticContext`. |
| `IncidentSignalReadModel` | read_models | Added to `DiagnosticContext` by ADR-001 §4. |
| `DiagnosticRule` | diagnostics | Type only — no implementors. |
| `ErasedSink` | notifications | **Not used** by orchestrator (looks legacy). |

**Designed in ADRs, not yet in code:**

| Trait | Module | Source |
|---|---|---|
| `IncidentRepository` | incidents | ADR-L4 §L4.6 |
| `SuppressionRepository` | incidents | ADR-L5 §L5.3 (V0.1) |
| `Projection` | read_models | ADR-R1 §R1.4 |
| `StateReadModelExt` | read_models | ADR-R1 §R1.3 (auto-impl extension for typed helpers) |
| `PollingCollector` | collectors | ADR-C1 |
| `SubscriptionCollector` | collectors | ADR-C1 (V0.1+ implementations) |

## 21.4 Commands / events found

**In code today:**

- `IncidentLifecycleEvent::{Opened, Escalated{…}, Resolved}` — defined and
  consumed by `Notifier::dispatch`.
- No `ObservationEvent`, `IncidentEvent`, or `NotificationEvent` types
  beyond the lifecycle one above.

**Designed in ADRs, not yet in code:**

- **Commands** (ADR-D3, full V0 vocabulary; V0.2 handlers stubbed):
  - `IncidentCommand::{RecordSignal, Acknowledge, Resolve}` (engine).
  - `SuppressionCommand::{Suppress, Unsuppress}` (separate
    `SuppressionService`).
- **Events** (ADR-D4, β events-only output; per-context modules):
  - `IncidentEvent::{SignalRecorded, IncidentTouched, Lifecycle,
    DraftRejected, DraftBelowConfidenceFloor}` — the engine's return
    type, replacing `HandleOutcome` (ADR-L4 §L4.2 superseded).
  - `ObservationEvent::{BatchProduced, ObservationAppended}`.
  - `ReadModelEvent::Applied`.
  - `DiagnosticEvent::{DraftEmitted, RuleFailed}`.
  - `NotificationEvent::{Dispatched, Suppressed}`.
  - Top-level `DomainEvent` envelope sums all of the above.
- **Errors**:
  - `EngineError::{Draft, NotYetImplemented}` (ADR-D3).
  - `SuppressionError::{NotYetImplemented, Repository}` (ADR-D3).
  - `ParseDottedNameError::{Empty, TooLong, BadCharacter,
    EmptySegment, BadSegmentStart, NoDot}` (ADR-D2).
  - `DraftError`, `RegistryError` (ADR-L1).

## 21.5 Storage abstractions found

**In code today:** None.

**Designed in ADRs, not yet in code:**

- `ObservationStore` trait (ADR-P2) — `append_many` + `iter_since`.
- `StoreError` (ADR-P2).
- `IncidentRepository` trait (ADR-L4 §L4.6).
- `SuppressionRepository` trait (ADR-L5 §L5.3; V0.1+).
- `NotificationAttemptRepository` trait (ADR-P3) — `insert_pending` +
  `complete` + `list_retryable` + `list_for_incident`.
- `SqliteObservationStore`, `SqliteIncidentRepository`,
  `SqliteSuppressionRepository` (V0.1), `SqliteNotificationAttemptRepository`
  — concrete sqlx impls per ADR-P1 + ADR-P3.
- `MemoryObservationStore`, `MemoryIncidentRepository`,
  `MemoryNotificationAttemptRepository` — test impls per ADR-P2 §P2.7
  and ADR-P3.
- `RetentionConfig` + `retention::run` (ADR-P2 §P2.5) — extended with
  `attempts_max_age` per ADR-P3 §P3.11.
- `migrations/0001_initial.sql` (ADR-P1, ADR-P3) — four tables.

## 21.6 Concrete collectors found

**In code today:** None. `src/collectors/traits.rs` is empty.

**Designed for V0** (per ADR-C3):

| Collector | Trait | Module | Status |
|---|---|---|---|
| `BitcoinCoreRpcCollector` | `PollingCollector` | `src/collectors/bitcoin_core/rpc.rs` | V0 target |
| `BitcoinRpcClient` (helper) | — | `src/collectors/bitcoin_core/rpc_client.rs` | V0 target |

**Designed for V0.1+:**

| Collector | Trait | Module |
|---|---|---|
| `BitcoinCoreZmqCollector` | `SubscriptionCollector` | `src/collectors/bitcoin_core/zmq.rs` |
| `LndGrpcPollCollector` | `PollingCollector` | `src/collectors/lnd/grpc_poll.rs` |
| `LndGrpcStreamCollector` | `SubscriptionCollector` | `src/collectors/lnd/grpc_stream.rs` |
| `LndRestCollector` | `PollingCollector` | `src/collectors/lnd/rest.rs` |
| `HostCollector` | `PollingCollector` | `src/collectors/host/mod.rs` |

## 21.7 Diagnostics found

None. `DiagnosticRule` trait exists; no implementors. The catalog
(`docs/INCIDENT_CATALOG.md`) lists ~17 candidate rules across Bitcoin Core,
LND, Elements, and cross-cutting categories.

## 21.8 Incident types

**In code today:**

- `Incident` aggregate type (will gain `fingerprint`; `signal_observation_ids` becomes `Vec` per ADR-001/L1).
- `IncidentKind(String)` — open vocabulary.
- `IncidentSeverity::{Info, Warning, Critical}`.
- `IncidentStatus::{Open, Acknowledged, Resolved, Suppressed}` (typo fix per ADR-001).
- `IncidentLifecycleEvent::{Opened, Escalated{previous_severity,
  new_severity}, Resolved}`.
- `IncidentNotificationEventKind::{Opened, Escalated, Resolved}`.

**Designed in ADRs L1–L5, not yet in code:**

- `IncidentFingerprint { subject, kind, dimension }`.
- `EntitySubjectKind { Host, BitcoinNode, BitcoinPeer, LndNode, LndPeer, LndChannel, LndInvoice }`
  (in `src/shared/types.rs`).
- `IncidentKindSpec { name, allowed_subjects, allows_dimension, dimension_label, min_open_confidence, source }`.
- `KindRegistry`, `KindSource::{Builtin, UserConfig}`.
- `RegistryError`, `DraftError`.
- `IncidentEngine`, `IncidentCommand::RecordSignal`, `HandleOutcome`, `EngineError`.
- `IncidentRepository` (trait), `RepoError`.
- `SuppressionRule`, `SuppressionRuleId`, `SuppressionRepository` (V0.1).
- `ActorId(String)` (V0.1).
- `well_known.rs` const string identifiers per built-in kind.

## 21.9 Notification types

**In code today:**

- Rich set — see § 21.2.
- All three target adapters (telegram/discord/webhook) have full type and
  rendering pipelines.
- All three senders are stubs returning `PermanentError::BadRequest("not
  yet implemented")`.

**Added in ADR-L5 (V0.1, not yet in code):**

- `DeliveryOutcome::Suppressed { rule_id: SuppressionRuleId }` — new variant.
- Notifier suppression-check step (`Notifier::dispatch` consults
  `SuppressionRepository` before fanning out).

**Added in ADR-P3 (V0, not yet in code):**

- `NotificationAttempt` revised: gains `incident_id`, `lifecycle_kind`,
  `target_kind`, `target_summary`, `attempt_number`,
  `parent_attempt_id`, `next_retry_at`, `outcome: Option<DeliveryOutcome>`,
  `external_ref`; loses the embedded full `IncidentLifecycleEvent` and
  full `NotificationTarget` (the latter carries secrets and must not
  be persisted).
- `NotificationAttemptRepository` trait — `insert_pending`, `complete`,
  `list_retryable`, `list_for_incident`.
- `NotificationDeliveryStatus` expanded: `Pending`, `Succeeded`,
  `FailedTransient`, `FailedPermanent`, `Suppressed`.
- `TargetKind` enum (Telegram, Discord, Webhook, Stdout).
- `Notifier::dispatch` signature change — accepts
  `&dyn NotificationAttemptRepository` and returns
  `Vec<NotificationAttempt>` instead of `Vec<(NotificationRuleId, DeliveryReceipt)>`.
- Retry scheduler tick in `runtime::consumer` (`select!` arm,
  10s default).

## 21.9.5 Runtime types (designed; not yet in code)

Per ADRs S1–S3:

- `RuntimeDeps` — bag of dependencies passed to `runtime::run()`.
- `RuntimeConfig` — channel capacity, timeouts, supervisor backoff.
- `RuntimeError` — top-level error type returned from `runtime::run()`.
- `runtime::supervisor` — collector task supervision + shutdown
  coordination.
- `runtime::consumer` — pipeline consumer task.
- `runtime::bootstrap` — `build_polling_collectors`,
  `build_subscription_collectors`.
- `runtime::rules::all()` — hand-wired rule registry.

## 21.10 Mismatches against the prior spec

Summarized in § 19. The biggest ones:

1. **Targets** — flat `TargetId` rejected; `EntityRef` enum used instead.
2. **Observations** — eight payload variants, not four.
3. **State** — strongly typed per subsystem, not `StateName + StateValue`.
4. **Attributes** — bounded enum, not JSON.
5. **Probe** — explicit batch + window + result with the
   "failed-implies-health" invariant.
6. **Diagnostics output** — `IncidentSignalDraft` with active/cleared
   status, not `DiagnosticFinding`.
7. **Incident lifecycle** — `Escalated{prev, new}` instead of generic
   `Updated`.
8. **Incident fingerprinting** — absent.
9. **Notification delivery** — much richer error taxonomy with retry hints
   and external message references.
10. **Stdout sink** — debug-only, not a V0 production sink.
11. **Runtime / storage / API** — none of these exist.

## 21.11 Proposed spec updates

This document is the updated spec.

---

# 22. Recommended Final Shape

The guiding constraint from the prior spec still holds, refined to match
how the code expresses it:

```text
Collectors observe (and emit batched probe results).
Observations record typed facts.
Read models project per-subject views with provenance.
Diagnostics interpret read models into incident signal drafts.
An incident engine lifts signals into durable incidents with lifecycle events.
The notifier matches lifecycle events to rules and dispatches to typed sinks.
The runtime coordinates the pipeline (and does not yet exist).
```

---

# 23. Architecture Decision Records

ADRs are appended in order. Each one captures: **Context** (what triggered
the decision), **Decision** (what we chose), **Rationale** (why), and
**Alternatives considered** (what we rejected and why). Decisions here
override conflicting statements earlier in this spec; the relevant
sections are updated in place when an ADR lands.

## ADR-001 — Incident-engine small calls

**Date.** 2026-05-17.
**Status.** Accepted.
**Scope.** Six low-risk cleanups blocking the incident engine work.

**Context.** The incident-engine design surfaced a handful of low-risk
issues — typos, missing exports, and gaps in trait surfaces — that don't
need brainstorming. Bundling them so the load-bearing brainstorms aren't
slowed down by them.

**Decision.**

1. **`IncidentStatus::Supressed` → `Suppressed`.** Pure spelling fix.
2. **`Incident.signal_observation_ids: Option<ObservationId>` → `Vec<ObservationId>`.**
   The field name was already plural; the type was wrong. One incident
   can accumulate multiple supporting signal observations across its
   lifetime.
3. **Export the `diagnostics` submodules.** `src/diagnostics/mod.rs` becomes
   `pub mod traits; pub mod types;` (or `pub use`-style), so
   `DiagnosticRule`, `DiagnosticContext`, and `IncidentSignalDraft` are
   reachable from `main.rs` and sibling modules.
4. **Add `signals: &'a dyn IncidentSignalReadModel` to `DiagnosticContext`.**
   Rules emitting `SignalStatus::Cleared` need to see what's currently
   Active for the same subject — otherwise they can clear signals they
   never raised.
5. **`IncidentKind` stays a `String` newtype.** Add `src/incidents/kinds.rs`
   with `pub const` items for the canonical V0 vocabulary (sourced from
   `docs/INCIDENT_CATALOG.md` — e.g. `BITCOIN_TIP_LAG`, `BITCOIN_PEER_STARVATION`,
   `LND_CHANNEL_INACTIVE`, …). Open vocabulary kept; canonical kinds become
   greppable constants instead of stringly-typed magic.
6. **Engine module layout.** Add `src/incidents/engine.rs` (service) and
   `src/incidents/repository.rs` (trait). Whether to also add
   `src/incidents/commands.rs` is deferred to ADR-L4 (engine surface area).

**Rationale.** All six are either bug fixes, missing exports, or naming
choices with one obviously-right answer.

**Alternatives considered.**

- For (5), switching `IncidentKind` to an enum was rejected: it would force
  every new diagnostic rule to touch a central enum, which fights the
  "rules ship independently" goal implied by the catalog's open shape and
  the `DiagnosticRule::id(&'static str)` choice.
- For (6), putting the engine inside `incidents/mod.rs` was rejected for
  the same module-per-file convention the rest of the crate follows.

**Spec updates.** § 10.1 / § 10.4 / § 9.2 / § 20.6 / § 21.2 / § 21.8
will be edited in-place after ADR-L1–L5 land, so we don't churn the spec
mid-brainstorm.

---

## ADR-L1 — Incident fingerprinting & kind registry

**Date.** 2026-05-17.
**Status.** Accepted.
**Scope.** Incident primary key, kind catalog source, draft validation.

**Context.** The incident engine needs a primary key to decide whether a
new `IncidentSignalDraft` opens a new incident or attaches to an
existing one. The prior spec assumed an `IncidentFingerprint`; the code
has none. Two related questions: how is the key shaped, and where does
the catalog of valid kinds live?

The `EntityRef` enum in `src/shared/types.rs` already factors monitored
subjects finely (host, bitcoin node, bitcoin peer, lnd node, lnd peer,
lnd channel, lnd invoice). That gets us most of the per-instance dedup
for free. The residual cases — multiple stuck HTLCs on one channel,
multiple full disks on one host — need an extension hatch.

**Decision.**

### 1. Fingerprint shape

```rust
pub struct IncidentFingerprint {
    pub subject: EntityRef,
    pub kind: IncidentKind,
    pub dimension: Option<String>,
}

impl IncidentFingerprint {
    pub fn as_key(&self) -> String { /* stable serialization for storage */ }
}
```

### 2. Signal draft additions

```rust
pub struct IncidentSignalDraft {
    pub subject: EntityRef,
    pub signal: SignalName,
    pub kind: IncidentKind,        // NEW
    pub dimension: Option<String>, // NEW
    pub severity: SignalSeverity,
    pub status: SignalStatus,
    pub confidence: Confidence,
    pub evidence: Vec<EvidenceRef>,
}
```

The rule sets `kind` and `dimension` per draft. The engine computes the
fingerprint on receipt from `(draft.subject, draft.kind, draft.dimension)`.
Rules never construct fingerprints directly.

### 3. Subject discriminant

```rust
// src/shared/types.rs
pub enum EntitySubjectKind {
    Host, BitcoinNode, BitcoinPeer,
    LndNode, LndPeer, LndChannel, LndInvoice,
}

impl EntityRef { pub fn subject_kind(&self) -> EntitySubjectKind; }
```

Named enum, not `std::mem::discriminant`: greppable, serializable, and
adding an `EntityRef` variant becomes a compile error in the spec table
instead of a silent miss.

### 4. Kind registry — built-in defaults + additive user config

Embedded TOML loaded at startup, with optional user-supplied TOML that
**adds** kinds. Overriding a built-in kind is an error.

```rust
// src/incidents/kinds.rs
pub struct IncidentKindSpec {
    pub name: String,
    pub allowed_subjects: Vec<EntitySubjectKind>,
    pub allows_dimension: bool,
    pub dimension_label: Option<String>, // documentation-only, not enforced
    pub source: KindSource,
}

pub enum KindSource { Builtin, UserConfig }

pub struct KindRegistry { kinds: HashMap<IncidentKind, IncidentKindSpec> }

impl KindRegistry {
    pub fn load(user_config: Option<&Path>) -> Result<Self, RegistryError>;
    pub fn lookup(&self, kind: &IncidentKind) -> Option<&IncidentKindSpec>;
    pub fn validate_draft(&self, draft: &IncidentSignalDraft) -> Result<(), DraftError>;
}

pub enum RegistryError {
    InvalidToml(String),
    DuplicateKind(IncidentKind),
    CannotOverrideBuiltin(IncidentKind),
    UnknownSubjectKind(String),
}

pub enum DraftError {
    UnknownKind(IncidentKind),
    DisallowedSubject {
        kind: IncidentKind,
        subject: EntitySubjectKind,
        allowed: Vec<EntitySubjectKind>,
    },
    DimensionRequired(IncidentKind),
    DimensionForbidden(IncidentKind),
}
```

Built-in defaults are embedded in the binary via
`include_str!("../../config/default_kinds.toml")`. A unit test asserts
parity between the TOML defaults and the `well_known.rs` string constants.

### 5. Built-in kind constants for internal rules

```rust
// src/incidents/well_known.rs
pub const BITCOIN_TIP_LAG: &str         = "bitcoin.tip_lag";
pub const BITCOIN_PEER_STARVATION: &str = "bitcoin.peer_starvation";
pub const LND_CHANNEL_INACTIVE: &str    = "lnd.channel_inactive";
pub const LND_HTLC_STUCK: &str          = "lnd.htlc_stuck";
pub const HOST_DISK_EXHAUSTION: &str    = "host.disk_exhaustion";
// …populated from docs/INCIDENT_CATALOG.md as rules land
```

Rules construct kinds via
`IncidentKind(well_known::BITCOIN_TIP_LAG.into())`.

### 6. Default kinds TOML shape

```toml
# config/default_kinds.toml — embedded in the binary
[[kinds]]
name = "bitcoin.tip_lag"
allowed_subjects = ["BitcoinNode"]
allows_dimension = false

[[kinds]]
name = "lnd.htlc_stuck"
allowed_subjects = ["LndChannel"]
allows_dimension = true
dimension_label = "payment_hash"

[[kinds]]
name = "host.disk_exhaustion"
allowed_subjects = ["Host"]
allows_dimension = true
dimension_label = "mount_path"
```

`dimension_label` is documentation: it tells operators what string the
rule will put in `dimension`. The registry does not validate the label
or the contents of `dimension`.

### 7. Validation timing

- **Startup.** `KindRegistry::load` validates TOML shape, rejects
  duplicate kinds, rejects user-config attempts to override built-ins,
  and rejects unknown subject-kind names.
- **Receipt-time.** `IncidentEngine` calls
  `KindRegistry::validate_draft` on every incoming draft. Validation
  failure rejects the draft; **no incident state is mutated**. The
  signal observation may still be persisted (`Origin::Computed`); only
  the incident lift is gated.

**Rationale.**

- **Why a structured fingerprint and not a free-form string** (Option C):
  rules can format strings inconsistently and the engine has no way to
  catch it. `(subject, kind, dimension)` keeps the structure in the type
  system.
- **Why per-draft `kind`** (not a static method on `DiagnosticRule`): a
  small number of rules will emit multiple incident kinds depending on
  which threshold tripped (e.g. an X1 disk rule could be `WARNING` at
  one threshold and `CRITICAL` at another, or could emit a `host.disk_io_errors`
  signal under a different kind entirely). Per-draft is more flexible;
  the registry catches mistakes.
- **Why config-driven kinds** (not in-code constants): operators are
  expected to ship rule libraries beyond the V0 catalog. A recompile is
  too high a bar.
- **Why additive-only** (Option δ, not γ override): overriding built-ins
  is a sharp footgun — an operator flips `allows_dimension` on a built-in
  and the rule emitting drafts for it suddenly fails validation. Better
  to make built-ins immutable from config.

**Alternatives considered.**

- **Option A** `(subject, kind)` only — rejected for losing per-HTLC and
  per-disk granularity.
- **Option C** rule-supplied string fingerprint — rejected for losing
  type discipline.
- **Option D** hash of evidence — rejected because evidence accumulates
  over an incident's lifetime, so the hash changes and dedup breaks.
- **Option β** config-only registry (no built-ins) — rejected because V0
  must work out of the box.
- **Option γ** built-ins with override — rejected for footgun risk.
- **Format JSON / YAML** — rejected; TOML is the Rust sidecar convention
  (matches `Cargo.toml`, `bithound.toml`).
- **`std::mem::discriminant(&EntityRef)` instead of `EntitySubjectKind`** —
  rejected for losing greppability and the compile-time check when
  `EntityRef` gains a variant.
- **Validating `dimension` content** (e.g. against a regex per kind) —
  deferred. `dimension_label` is documentation-only for now; if drift
  becomes a problem we add a richer `DimensionPolicy` enum later.

**Deferred / out of scope.**

- **Where the user-config TOML file lives** (`bithound.toml` section,
  separate `kinds.toml`, `kinds.d/` directory). The broader sidecar
  config mechanism has not been designed; `KindRegistry::load` accepts
  an `Option<&Path>` and lets the runtime decide.
- **Hot reload.** Registry is loaded once at startup. Reloading without
  restart can come later.
- **Plugin-supplied rules** registering their own kinds at runtime.
  Same answer — out of V0 scope.

**Spec updates queued** (folded after ADR-L5 lands):

- § 10.1 — add `fingerprint`, document `Vec<ObservationId>` for `signal_observation_ids`, fix `Suppressed`.
- § 10.4 — replace the "no design picked" note with the structured-key answer.
- § 9.2 — add `kind`, `dimension` to `IncidentSignalDraft`.
- § 21.8 — refresh incident-types inventory.
- New § 11.bis or appendix for the `KindRegistry` API.

---

## ADR-L2 — Signal-to-incident lift policy

**Date.** 2026-05-17.
**Status.** Accepted.
**Scope.** The engine's behavior when an `IncidentSignalDraft` arrives:
when to open, when to resolve, what to persist, how to handle edge cases.

**Context.** ADR-L1 settled how an incident is identified (fingerprint
shape + registry validation). It did not say *what the engine does* with
the validated draft. Five policy sub-questions had to be resolved before
the engine can be implemented.

**Decision.**

### L2.1 — Hysteresis is rule-owned

Rules look back through read models
(`MetricReadModel::metric_samples_since`, `unchanged_for`,
`StateReadModel::*`) and emit `Active` only once they've decided the
condition is real. The engine treats every Active draft as
immediate-open. Rules likewise emit `Cleared` once they've decided the
condition has ended.

The engine has no `min_consecutive_actives` knob, no debounce window, no
internal rule state. The reason this works: hysteresis windows vary
wildly by kind (A1 tip lag ≈ minutes, X1 disk exhaustion ≈ 24h
prediction window, B1 channel inactive ≈ 5–15 min wait), so a uniform
engine threshold is a footgun. A future `Hysteresis<T>` helper module
can be added if rules duplicate boilerplate.

### L2.2 — Confidence floor is a kind-spec knob, default `Medium`

`IncidentKindSpec` gains a field:

```rust
pub struct IncidentKindSpec {
    pub name: String,
    pub allowed_subjects: Vec<EntitySubjectKind>,
    pub allows_dimension: bool,
    pub dimension_label: Option<String>,
    pub min_open_confidence: Confidence,  // NEW (default Confidence::Medium)
    pub source: KindSource,
}
```

```toml
[[kinds]]
name = "bitcoin.tip_lag"
allowed_subjects = ["BitcoinNode"]
allows_dimension = false
min_open_confidence = "Medium"   # default; valid: "Low" | "Medium" | "High"
```

Drafts with `draft.confidence < spec.min_open_confidence` still persist
as `IncidentSignalObservation`s (so they're visible in dashboards and
read models), but **the incident-lift step is skipped** — no incident
is opened, no existing incident is touched.

The kind-spec location was chosen over an engine constant so operators
can dial sensitivity per kind through the same TOML mechanism ADR-L1
set up. No new config surface.

### L2.3 — Re-fire after Resolved creates a new incident

A draft with `Active` arrives for a fingerprint whose previous incident
has status `Resolved`. The engine **creates a new `Incident`** with a
fresh `IncidentId` and a fresh `opened_at`. The previous incident stays
Resolved with its own `resolved_at` preserved.

Reasons:
- Each Opened is a real "this stopped working again" event; chronic
  flappers should not show a misleading multi-week-old `opened_at`.
- Flap-noise mitigation belongs at the notifier (rate limits / digest
  mode), not at the incident layer.
- A kind-level "reopen same" policy can be added later for kinds where
  flap-collapsing is the right operator UX (B1 channel inactive is the
  obvious candidate). For now, all kinds get new-incident semantics.

### L2.4 — Multi-rule contribution is implicit for V0

When two rules emit drafts that compute to the same fingerprint, they
land on the same incident. The engine does not track which rule
contributed. Last status wins for transitions. **Convention is
"one rule owns one kind"** — if two rules need to contribute, they
should emit through different kinds.

Deferred: explicit per-`SignalName` contribution tracking and "all
contributors must clear" resolution semantics. Adopt when the first
real case appears.

### L2.5 — `Cleared` with no matching Active is persist-plus-no-op

A `Cleared` draft arrives for a fingerprint with no Open incident
(either never opened, or already Resolved). The engine:

- Persists the `IncidentSignalObservation` for read-model history.
- Does not mutate any incident state.
- Does not return an error to the caller.

Useful for debugging "why didn't this clear" and for keeping the
signal log consistent with what rules actually emitted. A rule that
emits Cleared on every tick is wasteful but not incorrect.

### Side decision — Validation failure rejects entirely

Updating ADR-L1's earlier note: a draft that fails
`KindRegistry::validate_draft` is **rejected outright** — no
`IncidentSignalObservation` is persisted, no incident state mutates,
and the engine returns `DraftError` to the caller. A malformed draft
should not pollute the observation log.

### Putting it together (decision diagram)

```text
draft arrives at engine
  ↓
KindRegistry::validate_draft
  rejected ──→ return DraftError, persist nothing
  ok
  ↓
compute fingerprint = (subject, kind, dimension)
  ↓
persist IncidentSignalObservation
  ↓
look up current Incident for this fingerprint
  ↓
draft.status = Active:
  no Incident exists, OR previous Incident is Resolved:
    if draft.confidence < kind_spec.min_open_confidence: no-op
    else:                                                 create new Incident, emit Opened
  Open Incident exists:
    append evidence; update severity per ADR-L3; emit Escalated only if rule applies

draft.status = Cleared:
  no Open Incident:           no-op
  Open Incident exists:       set status=Resolved, set resolved_at, emit Resolved
```

**Rationale.** Each sub-decision was made independently above. The
common thread: push policy down to where it has the most context (rules
own hysteresis; kind specs own confidence floor) and keep the engine's
mechanism minimal and predictable.

**Alternatives considered.** See L2.1–L2.5 inline alternatives.

**Spec updates queued.**

- § 9.2 — note that rules own hysteresis; cross-reference read-model
  history helpers.
- § 10.x (new) — engine decision diagram.
- ADR-L1's `IncidentKindSpec` shape — add `min_open_confidence`.

---

## ADR-L3 — Severity & escalation semantics

**Date.** 2026-05-17.
**Status.** Accepted.
**Scope.** How an incident's `severity` is computed across its lifetime
and when `IncidentLifecycleEvent::Escalated` is emitted.

**Context.** ADR-L2 left "what severity does the incident take, and when
does Escalated fire" unspecified. The existing
`IncidentLifecycleEvent::Escalated { previous_severity, new_severity }`
type implies up-direction, but doesn't enforce it.

**Decision.**

### L3.1 — Incident severity is monotonic max

```rust
incident.severity := MAX(incident.severity, draft.severity);
```

Once an incident has been Critical, it stays Critical for the rest of
its lifetime — a transient improvement does not degrade the operator's
view. Rules don't have to remember "the worst we've seen." This mirrors
PagerDuty / Sentry / OpsGenie behavior. Plays well with L2.4 implicit
multi-rule: if Rule A emits Warning and Rule B emits Critical for the
same fingerprint, the incident lands at Critical.

A future "current severity vs peak severity" pair can be added if
operators ask for it; for V0 we record only the peak.

### L3.2 — `Escalated` fires only on strict severity increase

```text
new_severity > previous_severity   → emit Escalated { previous, new }
new_severity == previous_severity  → no event (silent update)
new_severity < previous_severity   → impossible under L3.1
```

Severity rank: `Info < Warning < Critical` (already encoded in
`src/notifications/types.rs:164-174`). The engine reuses the same rank.

### L3.3 — `Opened` carries final initial severity

An incident's `severity` at creation equals the `severity` of the
`Active` draft that opened it. The `Opened` event carries the
already-correct severity — no separate "first Escalated" is fired on
creation. `Escalated` only fires on a *subsequent* upward move.

Canonical sequence:

```text
t0  Active@Warning  → no Open incident
    → create Incident{severity:Warning, status:Open, opened_at:t0}
    → emit Opened(incident)

t1  Active@Warning  → Open@Warning
    → append evidence, bump updated_at, NO event

t2  Active@Critical → Open@Warning
    → severity := Critical, bump updated_at
    → emit Escalated{prev:Warning, new:Critical}

t3  Active@Warning  → Open@Critical
    → MAX(Critical, Warning) = Critical, no change, bump updated_at, NO event

t4  Active@Info     → Open@Critical
    → no change, bump updated_at, NO event

t5  Cleared@*       → Open@Critical
    → status := Resolved, resolved_at := t5
    → emit Resolved(incident)
```

### L3.4 — Severity downgrade only through Resolved → new incident

Combined with ADR-L2's "new incident on re-fire," severity downgrade is
only observable across an Opened/Resolved boundary. A Critical that
resolves at t=10 followed by an Active@Warning at t=20 produces a *new*
Warning incident; the old Critical stays Resolved at Critical.

**Rationale.** Monotonic max + strict-increase Escalated combine to
give the engine a clean, predictable lifecycle: severity moves in one
direction within an incident, and lifecycle events fire only when
operator attention is warranted (new condition, worse condition,
resolution). Silent `updated_at` bumps keep the audit trail honest
without spamming notifications.

**Alternatives considered.**

- **Direct severity** (`incident.severity := draft.severity` every
  time) — rejected because brief drops to Warning during a Critical
  incident would generate downward Escalated events that operators
  would learn to ignore.
- **Escalated on any change** including downgrade — rejected because
  the variant name "Escalated" carries directional meaning and the
  notification side (DiscordSeverityPalette, severity rank) is
  inherently one-directional.
- **Manual-only Escalated** (operator clicks "escalate") — deferred.
  V0 has no operator UI; automatic escalation is the only path.

**Spec updates queued.**

- § 10.2 — annotate `IncidentLifecycleEvent::Escalated` with the
  strict-increase rule.
- § 9.x — engine decision diagram from L3.3 added to the engine
  section (after L4 lands the engine module).

---

## ADR-L4 — Engine surface area

**Date.** 2026-05-17.
**Status.** Accepted.
**Scope.** API shape, command vocabulary, output shape, state location,
sync/async, and write-failure semantics for `IncidentEngine`.

**Context.** ADRs L1–L3 specify what the engine *does* given a draft.
L4 settles the *how*: what method(s) callers invoke, what shape the
return value takes, what state the engine holds, and how it stays
consistent with the persistence layer.

**Decision.**

### L4.1 — Command enum + `handle`

```rust
pub enum IncidentCommand {
    RecordSignal(IncidentSignalDraft),
    // Acknowledge, Suppress, Unsuppress, Resolve — deferred (see L4.3)
}

impl IncidentEngine {
    pub fn handle(&mut self, cmd: IncidentCommand, now: DateTime<Utc>)
        -> Result<HandleOutcome, EngineError>;
}
```

Commands are first-class types: serializable for audit, replayable for
tests, single instrumentation point for metrics. Stream-processor and
free-method shapes were rejected (over-engineered / harder to evolve).

### L4.2 — `HandleOutcome` with three channels — **SUPERSEDED BY ADR-D4**

> **Superseded by ADR-D4.** This section originally defined
> `HandleOutcome { signal_observation, touched_incident, lifecycle_events }`
> as the engine's output. ADR-D4 replaces it with
> `handle(cmd) -> Result<Vec<IncidentEvent>, EngineError>` to support
> cross-process event consumers (cloud fleet management). See ADR-D4
> for the current shape; the rest of ADR-L4 (command enum,
> single-writer rule, sync engine, repository trait) is unaffected.
>
> Preserved below for historical context.

~~The engine is **pure policy**: it computes what should happen but
performs no I/O. The caller threads each channel to the appropriate
store:~~

```rust
// SUPERSEDED — see ADR-D4
pub struct HandleOutcome {
    pub signal_observation: Option<Observation>,         // → observation store
    pub touched_incident:   Option<Incident>,            // → incident repo
    pub lifecycle_events:   Vec<IncidentLifecycleEvent>, // → Notifier
}
```

~~Three channels because every state change needs persistence
(`touched_incident`), but not every state change is notify-worthy
(silent `updated_at` bumps per ADR-L3 are persisted but emit no
lifecycle event).~~

### L4.3 — V0 command set: `RecordSignal` only

The plausible full vocabulary —
`Acknowledge { id, by, at }`,
`Suppress { fingerprint, until, by }`,
`Unsuppress { fingerprint, by }`,
`Resolve { id, by, at, reason }`
— is deferred. V0 has no operator UI and no manual-action surface;
defining these commands without a caller is YAGNI. They are added in
V0.1 alongside the UI/CLI surface that triggers them, and L5 designs
the suppression semantics in detail.

`IncidentStatus::Acknowledged` and `IncidentStatus::Suppressed` remain
in the type for forward compatibility; they are unreachable in V0
because no command sets them.

### L4.4 — In-memory state, rebuilt at startup, single-writer, no periodic reconciliation

```rust
pub struct IncidentEngine {
    kinds: KindRegistry,
    sidecar_id: SidecarId,
    open_incidents: HashMap<IncidentFingerprint, Incident>,
}

impl IncidentEngine {
    pub fn new(kinds: KindRegistry, sidecar_id: SidecarId,
               open_incidents: Vec<Incident>) -> Self;
}
```

The runtime loads open incidents from the repository at startup
(`repo.load_open().await?`) and passes them to the constructor. After
that, the engine's `open_incidents` map is the live source of truth for
current decisions; the repository is the persistent follower.

**No periodic reconciliation between engine and repository.** In a
strict single-writer model — the runtime loop is the only caller of
`engine.handle` — reconciliation cannot fix the only realistic
divergence mode (a failed repo write after the engine mutated
in-memory state). In fact, periodic reconcile would *overwrite* the
engine's correct recent state with the repo's stale state, making the
problem worse.

**Write-through, not reconcile, is the consistency primitive.** The
runtime loop persists `outcome.touched_incident` to the repo *before*
acknowledging the signal observation or dispatching lifecycle events.
On repo write failure, the loop retries with backoff; if retries are
exhausted, the engine's rollback path (deferred to the runtime ADR
cluster) restores the in-memory state. This keeps both stores
consistent without a separate sync pass.

**Reconciliation becomes necessary when V0 ends:**

- Operator UI writing to the repo directly (V0.2+): preferred fix is
  routing UI writes through `engine.handle` as commands so the engine
  stays the sole mutator; reconciliation is the fallback if that's not
  feasible.
- HA / multi-sidecar (post-V0.2): full reconciliation or move to
  repo-as-authority with write locks.

These are explicitly out of V0 scope.

### L4.5 — Synchronous engine

The engine holds no I/O — it's pure policy over an in-memory `HashMap`.
Synchronous. The async work (loading from repo, persisting outcomes,
dispatching notifications) happens around it in the runtime loop.
Easier to test, no `Send + Sync` boilerplate, no async pollution into
the diagnostic context downstream.

### L4.6 — Concrete `IncidentEngine`, trait-backed `IncidentRepository`

```rust
// src/incidents/engine.rs           — concrete struct
pub struct IncidentEngine { /* … */ }
impl IncidentEngine { /* handle, new */ }

// src/incidents/repository.rs       — trait, multiple impls expected
#[async_trait]
pub trait IncidentRepository: Send + Sync {
    async fn load_open(&self) -> Result<Vec<Incident>, RepoError>;
    async fn save(&self, incident: &Incident) -> Result<(), RepoError>;
}

pub enum RepoError {
    Backend(String),
    Conflict { id: IncidentId },
    NotFound { id: IncidentId },
}
```

The engine is one implementation; pluggability lives at the repository
boundary (in-memory, JSONL, SQLite, …). Trait surface kept minimal —
`load_open` + `save` is enough for V0; richer queries get added as
needed.

**Rationale.** The unifying theme is keeping the engine *small and
pure*: one entry point, one return type with explicit channels, no
async, no I/O, no background loops. Everything else lives in the
runtime layer around it, where it can be tested and changed without
touching policy.

**Alternatives considered.** See L4.1, L4.2, L4.3, and the long
reconciliation discussion preceding this ADR (Path A / B / C).
Periodic reconciliation (Path C) was rejected as described in L4.4.
Two-phase prepare/commit/rollback (Path B) was rejected as too much
API surface for the failure mode V0 actually faces; basic retry on
write failure handles the realistic case.

**Spec updates queued.**

- § 10.x (new) — full engine module sketch.
- § 21.8 — add `IncidentCommand`, `HandleOutcome`, `EngineError`,
  `IncidentRepository`, `RepoError`, `KindRegistry`, `IncidentKindSpec`,
  `KindSource`, `RegistryError`, `DraftError`, `EntitySubjectKind`,
  `IncidentFingerprint` to the inventory.
- § 12 — refresh the "no runtime" note: the engine module is now
  designed even though it's still unimplemented, and the runtime's
  responsibility is enumerated (load, handle, persist outcome, dispatch
  events, retry on write failure).

---

## ADR-L5 — Suppression model

**Date.** 2026-05-17.
**Status.** Accepted (design); deferred (V0 implementation).
**Scope.** Suppression rule shape, scope, behavior, integration with
the notifier, and V0/V0.1/V0.2 capability split.

**Context.** `IncidentStatus::Suppressed` already exists in
`src/incidents/types.rs` (renamed from `Supressed` in ADR-001). ADR-L4
deferred the `Suppress`/`Unsuppress` commands. The design needs to
serve two real operator use cases:

- **Known-issue muting.** "Stop paging me about this recurring B1
  channel-inactive for the next 4 hours."
- **Maintenance windows.** "Patching `host-alice` from 02:00–04:00 UTC;
  mute everything on that subject during the window."

The design must specify behavior so V0.1 can implement it without
re-deciding.

**Decision.**

### L5.1 — Per-fingerprint rules; maintenance windows expand at config-load

The on-the-wire suppression model is a flat set of
`SuppressionRule { fingerprint, … }` rules. Wildcards are not
represented at the rule level. Maintenance-window configuration
(scheduled by subject, kind, etc.) is **expanded into N
per-fingerprint rules at config-load time**, against the catalog of
active incidents at the window start.

- O(1) lookup per draft (HashMap by fingerprint).
- Wildcard expressiveness lives in the config layer, not the storage
  layer — a clear separation that lets us add matcher-based rules
  later if operators require them.
- Limitation: a maintenance window cannot suppress *new* incident
  kinds that open *during* the window. Acceptable for V0.1 (operators
  can explicitly add a rule for newcomers; matcher rules are V0.3+).

### L5.2 — Notifier-side filtering; `IncidentStatus::Suppressed` stays vestigial in V0.1

Suppression is purely a notification concern. The engine ignores
suppression rules entirely — opens, escalates, and resolves incidents
exactly per ADRs L1–L4. The notifier consults a
`SuppressionRepository` before each dispatch and drops events matching
active rules. This keeps engine policy unchanged and preserves the full
incident audit trail (dashboards / signal read models / incident repo
all reflect what happened during a suppressed window).

`IncidentStatus::Suppressed` stays in the enum but is **not set by the
V0.1 engine**. It is reserved for a future V0.2 semantic distinction:

- *Operator-acknowledged-known* — sets `Incident.status = Suppressed`
  on a specific incident (via a future `Acknowledge`-shaped command).
- *Operator-or-schedule-muted* — adds a `SuppressionRule`; does not
  touch any incident's status.

Different mechanisms, different audit semantics, both useful.

### L5.3 — Rule shape

```rust
// src/incidents/suppression.rs
pub struct SuppressionRuleId(pub Uuid);

impl SuppressionRuleId {
    pub fn new() -> Self { Self(Uuid::now_v7()) }
}

pub struct SuppressionRule {
    pub id: SuppressionRuleId,
    pub fingerprint: IncidentFingerprint,
    pub until: Option<DateTime<Utc>>,      // None = indefinite
    pub reason: String,                    // operator-visible
    pub by: ActorId,                       // "system" for maintenance windows
    pub created_at: DateTime<Utc>,
}

#[async_trait]
pub trait SuppressionRepository: Send + Sync {
    async fn list_active(&self, now: DateTime<Utc>)
        -> Result<Vec<SuppressionRule>, RepoError>;
    async fn matches(&self, fingerprint: &IncidentFingerprint, now: DateTime<Utc>)
        -> Result<Option<SuppressionRuleId>, RepoError>;
    async fn add(&self, rule: SuppressionRule) -> Result<(), RepoError>;
    async fn remove(&self, id: SuppressionRuleId) -> Result<(), RepoError>;
}
```

`matches` returns `Option<SuppressionRuleId>` rather than `bool` so the
notifier can record which rule muted the event. `until = None` means
indefinite suppression (operator must explicitly unsuppress).
`until = Some(t)` is auto-expiring; the repository implementation is
responsible for filtering by `now`.

### L5.4 — Notifier integration with auditable suppression receipts

A new `DeliveryOutcome` variant captures suppressed deliveries so the
future operator UI can answer "which rule muted this?":

```rust
pub enum DeliveryOutcome {
    Delivered { external_ref: Option<ExternalMessageRef> },
    Transient { error: TransientError, retry_after: Option<Duration> },
    Permanent { error: PermanentError },
    Suppressed { rule_id: SuppressionRuleId },     // NEW
}
```

`Notifier::dispatch` becomes (sketch):

```rust
pub async fn dispatch(&self, event: &IncidentLifecycleEvent, message: &NotificationMessage)
    -> Vec<(NotificationRuleId, DeliveryReceipt)>
{
    let fp = compute_fingerprint(event.incident());
    if let Some(rule_id) = self.suppression.matches(&fp, Utc::now()).await.unwrap_or(None) {
        // synthesize a Suppressed receipt for every matching notification rule
        return self.rules.iter()
            .filter(|r| r.matches(event))
            .map(|r| (r.id.clone(), suppressed_receipt(rule_id.clone())))
            .collect();
    }
    // existing dispatch logic
}
```

This keeps the rule-matching tally complete (every rule that *would*
have fired still appears in the result), preserves an audit point per
delivery, and gives the operator UI a single shape to render.

### L5.5 — ActorId strawman

```rust
// src/shared/types.rs
pub struct ActorId(pub String);

impl ActorId {
    pub fn system() -> Self { Self("system".into()) }
    pub fn operator(name: impl Into<String>) -> Self { Self(name.into()) }
}
```

Plain `String` newtype with two named constructors. Richer modelling
(real user identity, RBAC, audit) is deferred to V0.2 when the
operator UI introduces actual users. The shape is forward-compatible:
`ActorId` can become an enum without breaking serialization.

### L5.6 — Module placement

`SuppressionRule` and `SuppressionRepository` live in
**`src/incidents/suppression.rs`**, not `src/notifications/`.

Reason: a suppression rule is keyed on `IncidentFingerprint`, which is
an incident-domain concept (defined in `src/incidents/types.rs` per
ADR-L1). The notifier *consumes* the repository at dispatch time but
does not own the model. This mirrors `IncidentRepository` living in
incidents and being consumed by the runtime loop.

### L5.7 — V0 / V0.1 / V0.2 capability split

| Capability | V0 | V0.1 | V0.2 |
|---|---|---|---|
| `IncidentStatus::Suppressed` in the enum | yes (vestigial) | yes (vestigial) | active |
| `SuppressionRule` + `SuppressionRepository` | no | yes | yes |
| Notifier-side filtering + `DeliveryOutcome::Suppressed` | no | yes | yes |
| Per-fingerprint suppression via maintenance-window TOML | no | yes | yes |
| `IncidentCommand::Suppress` / `Unsuppress` | no | no | yes |
| Manual `IncidentStatus::Suppressed` via Acknowledge-style command | no | no | yes |
| Matcher-based / layered suppression | no | no | no — V0.3+ |

V0 ships none of this beyond the renamed enum variant. ADR-L5 captures
the design so V0.1 can implement it without re-deciding.

**Rationale.** Notifier-side filtering plus per-fingerprint rules is
the smallest design that serves both operator use cases honestly. It
keeps the engine pure, the notifier's responsibility clean, and the
audit trail intact. Maintenance-window expressiveness is pushed into
the config layer, where it's a feature rather than a runtime cost.

**Alternatives considered.**

- **Engine-side suppression** (engine sets `Open → Suppressed` on
  matching drafts) — rejected: forces engine to know about
  suppression, breaks the "engine is pure policy" rule from ADR-L4,
  and complicates the lifecycle event model.
- **Matcher-based rules** (`Option<EntityRef>` + `Option<IncidentKind>`
  wildcards) — deferred to V0.3+: needed only if maintenance-window
  TOML expansion proves insufficient.
- **Layered rules** (per-fingerprint + per-subject + per-kind tables)
  — rejected for the same reason as matchers, but more strongly:
  three storage paths is too much surface for an unproven need.
- **`DeliveryOutcome::Suppressed` as a `Permanent` variant** —
  rejected: suppression is a policy decision, not a delivery
  failure. A separate variant matches the actual semantics.

**Deferred / out of scope.**

- Maintenance-window TOML schema and where it lives (likely
  `bithound.toml [[maintenance_windows]]`). Tied to the broader sidecar
  config decision deferred in ADR-L1.
- Suppression rule garbage collection (expired `until` values).
  Implementation detail of the repo; trivial.
- Re-entrancy / rule add during an in-flight dispatch. Single-writer
  contract (per ADR-L4) makes this a non-issue.

**Spec updates queued.**

- § 10.1 — annotate `IncidentStatus::Suppressed` as "reserved for V0.2."
- § 11.x — add notifier suppression-check step and
  `DeliveryOutcome::Suppressed` variant.
- § 21.8 / § 21.9 — add `SuppressionRule`, `SuppressionRepository`,
  `SuppressionRuleId`, `ActorId`, `DeliveryOutcome::Suppressed` to the
  inventory.

---

## ADR-R1 — Read-model architecture

**Date.** 2026-05-17.
**Status.** Accepted.
**Scope.** How an `Observation` becomes a read-model entry, how the
read-model trait surface is organized, and how new projections are
added.

**Context.** The read-model trait set in `src/read_models/traits/`
exposes queries to diagnostic rules, but nothing in the codebase
populates the read models from observations. The original
`StateReadModel` further had per-state-variant methods
(`bitcoin_blockchain(node)`, `bitcoin_mempool(node)`, …), which
conflates "what a collector produces" with "what the read-model trait
exposes." Both issues had to be resolved before diagnostics can run
end-to-end and before the project can accept contributions cleanly.

**Decision.**

### R1.1 — Read-model traits are generic over observation type

Each read-model trait covers **one observation payload type**
generically. Sub-variants (BitcoinBlockchainState, LndNodeState, …) are
collector-side concerns; the read model returns the typed payload
generically and consumers pattern-match.

**`StateReadModel` is rewritten** to remove the per-variant methods:

```rust
// src/read_models/traits/state.rs
pub trait StateReadModel: Send + Sync + std::fmt::Debug {
    /// Latest state observation of a given name for a given subject.
    fn latest_state(&self, subject: &EntityRef, name: &StateName)
        -> Option<Projected<StateObservation>>;

    /// All known state observations for a subject.
    fn states_for(&self, subject: &EntityRef)
        -> Vec<Projected<StateObservation>>;
}
```

The other five trait shapes are already correct:
`MetricReadModel` keyed by `MetricName`, `HealthReadModel` by
`HealthTargetId`, `CapabilityReadModel` by `CapabilityName`,
`HeartbeatReadModel` sidecar-scoped, `IncidentSignalReadModel` by
`SignalName` / `IncidentKind`.

### R1.2 — Canonical state names

Each `StateObservation` variant gets a canonical name accessible via
a `name()` method:

```rust
// src/observations/types/state.rs
impl StateObservation {
    pub fn name(&self) -> StateName {
        StateName(match self {
            Self::BitcoinBlockchain(_)  => "bitcoin.blockchain",
            Self::BitcoinMempool(_)     => "bitcoin.mempool",
            Self::BitcoinNetwork(_)     => "bitcoin.network",
            Self::BitcoinPeerSummary(_) => "bitcoin.peer_summary",
            Self::LndNode(_)            => "lnd.node",
            Self::LndWallet(_)          => "lnd.wallet",
            Self::LndChannelSummary(_)  => "lnd.channel_summary",
            Self::Host(_)               => "host.system",
        }.to_string())
    }
}

// src/observations/types/state/well_known.rs
pub const BITCOIN_BLOCKCHAIN:    &str = "bitcoin.blockchain";
pub const BITCOIN_MEMPOOL:       &str = "bitcoin.mempool";
pub const BITCOIN_NETWORK:       &str = "bitcoin.network";
pub const BITCOIN_PEER_SUMMARY:  &str = "bitcoin.peer_summary";
pub const LND_NODE:              &str = "lnd.node";
pub const LND_WALLET:            &str = "lnd.wallet";
pub const LND_CHANNEL_SUMMARY:   &str = "lnd.channel_summary";
pub const HOST_SYSTEM:           &str = "host.system";
```

A unit test asserts parity between `well_known::*` and the
`StateObservation::name()` arms — adding a variant without updating
both fails the build.

### R1.3 — Optional `StateReadModelExt` for typed helpers

To keep pattern-matching boilerplate out of rules, an additive
extension trait is auto-implemented for any `StateReadModel`:

```rust
// src/read_models/traits/state_ext.rs
pub trait StateReadModelExt: StateReadModel {
    fn bitcoin_blockchain(&self, node: &BitcoinNodeId)
        -> Option<Projected<BitcoinBlockchainState>>
    {
        let proj = self.latest_state(
            &EntityRef::BitcoinNode(node.clone()),
            &StateName(well_known::BITCOIN_BLOCKCHAIN.into()),
        )?;
        if let StateObservation::BitcoinBlockchain(s) = proj.value {
            Some(Projected {
                value: s,
                observation_id: proj.observation_id,
                observed_at: proj.observed_at,
            })
        } else { None }
    }
    // … one helper per variant; lives next to the core trait,
    //   contributors add a helper when they add a variant.
}

impl<T: StateReadModel + ?Sized> StateReadModelExt for T {}
```

Rules use the extension if convenient and the generic method if not.
The core trait never changes when state variants are added.

### R1.4 — Architecture: six projections behind a thin store

The implementation lives in `src/read_models/projections/`, one
module per observation type that has a read-model trait. Each
projection owns its slice of state and a single `Projection` trait
(common shape only):

```rust
// src/read_models/projections/mod.rs
pub mod state;
pub mod metric;
pub mod health;
pub mod capability;
pub mod heartbeat;
pub mod incident_signal;

pub trait Projection: Send + Sync + std::fmt::Debug {
    fn apply(&mut self, obs: &Observation) -> Result<(), ProjectionError>;
}

#[derive(Debug)]
pub enum ProjectionError {
    InvalidPayload(String),
    InternalConsistency(String),
}
```

Each projection is self-contained (state + apply + query helpers).
The store is a thin assembler with typed fields — no registry, no
trait-object dispatch:

```rust
// src/read_models/store.rs
pub struct ReadModelStoreConfig {
    pub metric_history_capacity: usize,        // default 1000
    pub heartbeat_history_capacity: usize,     // default 256
}

pub struct ReadModelStore {
    pub state:           projections::state::StateProjection,
    pub metric:          projections::metric::MetricProjection,
    pub health:          projections::health::HealthProjection,
    pub capability:      projections::capability::CapabilityProjection,
    pub heartbeat:       projections::heartbeat::HeartbeatProjection,
    pub incident_signal: projections::incident_signal::IncidentSignalProjection,
}

#[derive(Debug)]
pub enum ApplyError {
    Projection(ProjectionError),
}

impl ReadModelStore {
    pub fn new(config: ReadModelStoreConfig) -> Self;
    pub fn apply(&mut self, obs: &Observation) -> Result<(), ApplyError> {
        match &obs.payload {
            ObservationPayload::State(_)          => self.state.apply(obs),
            ObservationPayload::Metric(_)         => self.metric.apply(obs),
            ObservationPayload::Health(_)         => self.health.apply(obs),
            ObservationPayload::Capability(_)     => self.capability.apply(obs),
            ObservationPayload::Heartbeat(_)      => self.heartbeat.apply(obs),
            ObservationPayload::IncidentSignal(_) => self.incident_signal.apply(obs),
            ObservationPayload::Event(_)
            | ObservationPayload::Inventory(_)
            | ObservationPayload::Transition(_)
            | ObservationPayload::Diagnosis(_)    => Ok(()),
        }.map_err(ApplyError::Projection)
    }
}

// Trait impls delegate to the projection field they belong to:
impl StateReadModel       for ReadModelStore { /* delegates to self.state */ }
impl MetricReadModel      for ReadModelStore { /* delegates to self.metric */ }
impl HealthReadModel      for ReadModelStore { /* delegates to self.health */ }
impl CapabilityReadModel  for ReadModelStore { /* delegates to self.capability */ }
impl HeartbeatReadModel   for ReadModelStore { /* delegates to self.heartbeat */ }
impl IncidentSignalReadModel for ReadModelStore { /* delegates to self.incident_signal */ }
```

### R1.5 — Contributor story

- **New collector emits an existing state shape** (e.g. CLN collector
  produces an `LndNodeState`-shaped observation): zero changes to the
  read model.
- **New state shape** (e.g. `ClnNodeState`): one variant added to
  `StateObservation` + one entry in `StateObservation::name()` + one
  `well_known` const. Existing `StateProjection` handles it
  generically. Optionally one helper in `StateReadModelExt`.
- **New observation type entirely** (e.g. `TraceObservation`): payload
  variant + new `TraceProjection` + one field on `ReadModelStore` + new
  `TraceReadModel` trait + one line in `DiagnosticContext`. This is the
  heaviest case but correctly central — it's a new domain concept.

**Rationale.**

- **Generic read-model traits** match how observations are actually
  produced: collectors decide the shape; the read model is a uniform
  query surface across shapes. The original per-variant `StateReadModel`
  methods leaked collector-specific structure into the query layer.
- **Per-projection modules** mean contributions touch one file per new
  projection, not a central mega-struct.
- **Typed fields on the assembler** keep dispatch zero-cost and queries
  type-checked at compile time. No `dyn Projection` indirection.
- **Extension trait for typed helpers** keeps rule call-sites
  ergonomic without polluting the core trait.

**Alternatives considered.**

- **Single concrete mega-store** (original proposal) — rejected:
  every new projection means editing a central struct; bad for
  contributions.
- **Projector registry with `Vec<Box<dyn Projection>>`** — rejected:
  pays for runtime composition we don't need; loses typed access at the
  query layer.
- **Event-sourced replay** as the only mechanism — rejected: needs an
  observation log designed first; cold-start works for V0.
- **Keep per-variant methods on `StateReadModel`** — rejected per
  the structural correction: variants are collector-side, not
  read-model-side.

**Spec updates queued.**

- § 8.2 — rewrite trait list to show generic `StateReadModel`.
- § 8.x (new) — store + projection module sketch.
- § 9.2 — show generic state query in `DiagnosticContext` usage.
- § 21.3 — refresh trait inventory.

---

## ADR-R2 — Derived observations as `ObservationPayload` variants

**Date.** 2026-05-17.
**Status.** Accepted.
**Scope.** Whether `IncidentSignalObservation` and `DiagnosisObservation`
(defined in code but not in the `ObservationPayload` enum) become
first-class payload variants.

**Context.** The code defines `IncidentSignalObservation` (in
`src/observations/types/incident_signal.rs`) and `DiagnosisObservation`
(in `src/observations/types/diagnosis.rs`) but neither appears in the
`ObservationPayload` enum. ADR-L1 flagged this as an open question.

The engine (ADR-L4) emits `signal_observation: Option<Observation>` as
part of `HandleOutcome`; that observation must flow through the same
store ingestion path as primary observations so `IncidentSignalReadModel`
can serve it. That forces a decision now.

**Decision.**

Add both as `ObservationPayload` variants:

```rust
pub enum ObservationPayload {
    Capability(CapabilityObservation),
    Diagnosis(DiagnosisObservation),         // NEW
    Event(EventObservation),
    Heartbeat(HeartbeatObservation),
    Health(HealthCheckObservation),
    IncidentSignal(IncidentSignalObservation), // NEW
    Inventory(InventoryObservation),
    Metric(MetricObservation),
    State(StateObservation),
    Transition(TransitionObservation),
}
```

The engine wraps a produced signal in a full `Observation` envelope
with:

- `source.collector = CollectorRef { id: CollectorId("incident-engine".into()), … }`
- `origin = ObservationOrigin::Computed`

Both `Computed` (existing) and the wrapping pattern keep derived
observations indistinguishable in shape from collector-produced ones,
flowing through the same `apply` path in `ReadModelStore` and the
same observation log.

`Observation::incident_signal(ctx, …)` and `Observation::diagnosis(ctx, …)`
constructor helpers are added alongside the existing eight, for
symmetry.

**Rationale.**

- **One ingestion path.** The store gets a single `apply(&Observation)`
  method; the engine and collectors both produce `Observation`s.
- **Provenance preserved.** The `ObservationSource` + `ObservationOrigin`
  fields already model derived data; using them for signal/diagnosis
  observations is the design they were built for.
- **DiagnosisObservation for forward compat.** No diagnosis rule emits
  one yet, but defining the variant now means richer findings can land
  later without enum churn or downstream coordination.

**Alternatives considered.**

- **Separate channel** (engine emits raw `IncidentSignalObservation`,
  store gets a second `apply_signal` method) — rejected: bifurcates
  the ingestion path, loses the `Observation` envelope's provenance,
  and forces every consumer to handle two shapes.
- **Add `IncidentSignal` only, defer `Diagnosis`** — rejected for
  symmetry and to avoid a second enum-churn PR when diagnosis arrives.

**Spec updates queued.**

- § 6.3 — list ten payload variants (was eight).
- § 21.2 — add `Diagnosis`, `IncidentSignal` to the inventory.

---

## ADR-R3 — Store small calls

**Date.** 2026-05-17.
**Status.** Accepted.
**Scope.** Five small implementation calls for the read-model store.

**Decision.**

1. **`apply` as a method on `ReadModelStore`**, not a trait yet.
   A `ReadModelStore` trait can be extracted later when a second
   concrete impl exists (e.g. SQLite-backed). For V0, one concrete
   struct is enough.

2. **`apply` is synchronous, `&mut self`.**
   The store holds in-memory state only — no I/O on the hot path. Sync
   means no `Send + Sync` boilerplate, easier testing, no async
   pollution into the diagnostic context. `&mut self` enforces
   single-writer through the type system; the runtime loop owns the
   store and calls apply + queries serially per tick. When the runtime
   grows to parallel collectors (V0.1+), wrap with `Arc<RwLock<…>>` or
   move to arc-swap snapshots — the trait surface doesn't change.

3. **Dispatch by `ObservationPayload` variant** (sketch in ADR-R1 §R1.4).
   Variants without a corresponding read-model trait in V0 (Event,
   Inventory, Transition, Diagnosis) are no-ops at the projection
   layer. They are still persisted to the observation store — that's
   a separate concern.

4. **Metric history is a bounded ring per `(subject, MetricName)`**,
   default 1000 samples, configurable via `ReadModelStoreConfig`.
   Eviction is FIFO. Heartbeat history is also a bounded ring,
   default 256 samples. State / health / capability / incident-signal
   projections keep only the latest value per key.

5. **Cold start in V0; replay in V0.1.**
   V0: the store starts empty after sidecar restart. Diagnostics
   return `None` until backfilled by incoming observations. Acceptable
   because polling intervals are sub-minute and the loss window is
   one or two ticks. V0.1: add
   `ReadModelStore::restore_from(impl Iterator<Item = Observation>)`
   and have the runtime call it with the last N minutes from the
   observation store at startup. No architectural change.

6. **Module location:** `src/read_models/store.rs` for the assembler,
   `src/read_models/projections/<name>.rs` for each projection,
   `src/read_models/traits/<name>.rs` for the query traits (unchanged).

**Rationale.** Each item independent; common thread is "smallest
shape that works for V0, easiest evolution path to V0.1+."

**Spec updates queued.**

- § 8.x — full module sketch.
- § 8.4 — refresh the "open questions" note: update path resolved.

---

## ADR-C1 — Two collector traits

**Date.** 2026-05-17.
**Status.** Accepted.
**Scope.** The trait shape that collectors implement.

**Context.** `src/collectors/traits.rs` is empty. `IntegrationKind`
already partitions collectors into polling vs subscription
(`CollectorMode::{Polling, Subscription}` in
`src/collectors/types.rs:51-54`). The lifecycle, timeout policy, and
restart semantics of the two modes differ enough that a single trait
would hide important distinctions.

**Decision.**

Define **two object-safe traits**, one per mode:

```rust
// src/collectors/traits.rs
#[async_trait]
pub trait PollingCollector: Send + Sync {
    fn descriptor(&self) -> &CollectorDescriptor;

    /// Run one collection pass. Returns a batch whose ProbeResult encodes
    /// success or failure; never returns Err.
    async fn poll(&self, ctx: CollectionContext) -> ObservationBatch;
}

#[async_trait]
pub trait SubscriptionCollector: Send + Sync {
    fn descriptor(&self) -> &CollectorDescriptor;

    /// Run until the subscription dies or the sink is closed. Returns
    /// Err if the connection died unrecoverably; runtime decides whether
    /// to re-spawn with backoff.
    async fn run(&self, ctx: CollectionContext, sink: BatchSink)
        -> Result<(), CollectionError>;
}

/// Handle the runtime gives subscription collectors so they emit
/// batches as data arrives. Internally wraps a tokio mpsc Sender.
pub struct BatchSink { /* … */ }

impl BatchSink {
    pub async fn send(&self, batch: ObservationBatch) -> Result<(), SinkError>;
}

pub enum SinkError {
    Closed,
}
```

The runtime holds two collections — `Vec<Box<dyn PollingCollector>>` and
`Vec<Box<dyn SubscriptionCollector>>` — and schedules each appropriately:
polling collectors get a tick at their declared `IntegrationKind`
interval; subscription collectors get spawned once with a `BatchSink`
and re-spawned with backoff on `Err`.

**V0 implementation scope.** Only `PollingCollector` is implemented
concretely. `SubscriptionCollector` is defined so the runtime ADR
cluster has the shape to design against; `BitcoinCoreZmqCollector` and
`LndGrpcStreamCollector` wait until V0.1+.

**Rationale.**

- **Lifecycle semantics differ.** Polling is request/response per tick;
  subscription is a long-lived stream. The same trait surface would
  either lie about one or force every caller to handle both flavors.
- **Object-safety preserved.** `async_trait` keeps both traits dyn-able,
  enabling `Vec<Box<dyn …>>` in the runtime.
- **Scheduling policy is mode-specific.** The scheduler ADR cluster
  (post-collectors) will encode "poll every N seconds" for polling
  collectors and "restart on failure with backoff" for subscription
  collectors — easier when the trait split is explicit.

**Alternatives considered.**

- **One unified trait** (`async fn run(ctx, sink) -> Result<…>`):
  rejected because polling collectors don't naturally need a sink, and
  forcing them through one obscures their per-tick batch contract.
- **One trait with two methods** (`poll` OR `run`, default-impl-or-panic):
  rejected as confusing — convention-based rather than type-enforced.

**Spec updates queued.**

- § 7.3 — replace "empty trait file" with the two-trait shape.
- § 21.3 — add `PollingCollector`, `SubscriptionCollector`, `BatchSink` to trait inventory.

---

## ADR-C2 — Polling output is `ObservationBatch` directly

**Date.** 2026-05-17.
**Status.** Accepted.
**Scope.** What `PollingCollector::poll` returns.

**Context.** Two plausible return shapes:
`ObservationBatch` directly, or `Result<ObservationBatch, CollectionError>`.
The latter creates two error channels (the outer `Err` and the batch's
`ProbeResult::Failed`) with ambiguous boundaries.

**Decision.**

```rust
async fn poll(&self, ctx: CollectionContext) -> ObservationBatch;
```

No outer `Result`. Every poll produces some batch. Probe failures are
encoded inside the batch via `ProbeResult::Failed { health,
partial_observations, error }` — the existing shape already says
exactly what failed and what was collected before the failure.

The collector contract:

1. `poll` never panics.
2. `poll` never returns `Err` (it can't — no `Result`).
3. Every internal error is mapped to a `CollectionError` and wrapped
   into `ProbeResult::Failed` with a `HealthCheckObservation`.
4. Observations collected before the failure are preserved in
   `ProbeResult::Failed.partial_observations`.

The runtime then has a uniform path: receive batch → append observations
→ run diagnostics. Failed batches contribute their health observation
and partials to the read models the same as successful ones.

Subscription collectors keep `Result<(), CollectionError>` on `run`
because there a terminal error (the connection died) is meaningfully
distinct from a stream of probe results.

**Rationale.**

- **One error channel is simpler.** The runtime doesn't have to decide
  "did the collector return Err, or Ok(failed batch)" — the answer is
  always the second form.
- **Partial failures are first-class.** If the third of four RPCs fails,
  the first two observations still land in the read models. With an
  outer `Result`, the natural temptation is to short-circuit on first
  error and lose them.
- **Health observations cover the "what happened" axis.** A failed
  probe always carries a `HealthCheckObservation` that diagnostics can
  reason about via `HealthReadModel`. The collector doesn't need a
  separate error return because the health observation IS the error
  channel.

**Alternatives considered.**

- **`Result<ObservationBatch, CollectionError>`** — rejected as
  duplicating the failure-modeling already done by `ProbeResult`.
- **`Result<ObservationBatch, BatchAssemblyError>`** for invariant
  violations (e.g. `ProbeWindow::Inverted`) — rejected because those
  are programmer errors, not runtime errors; `expect()` is the right
  failure mode and indicates a bug.

**Spec updates queued.**

- § 7.3 — annotate the contract.
- § 7.5 — note the `RpcError → CollectionErrorKind` mapping for
  Bitcoin Core RPC.

---

## ADR-C3 — Collector small calls

**Date.** 2026-05-17.
**Status.** Accepted.
**Scope.** Eight low-risk implementation calls for the collector
layer, including the first concrete collector
(`BitcoinCoreRpcCollector`).

**Decision.**

### C3.1 — Add `sidecar_id: SidecarId` to `CollectionContext`

```rust
pub struct CollectionContext {
    pub sidecar_id: SidecarId,                 // NEW
    pub collector_id: CollectorId,
    pub target: CollectorTarget,
    pub now: DateTime<Utc>,
    pub run_id: CollectionRunId,
}
```

The collector needs `sidecar_id` to stamp `ObservationBatch.sidecar_id`
and `ObservationSource.sidecar_id` on emitted observations. Threading
it through `CollectionContext` keeps the call site clean and lets the
runtime own sidecar identity in one place.

### C3.2 — Free constructors per collector; runtime hand-wires

No `CollectorFactory` trait, no registry. Each integration exposes a
plain `new(descriptor, connection, http) -> Result<Self, BuildError>`
constructor. The runtime iterates over `CollectorDescriptor`s, matches
on `IntegrationKind`, and constructs the right collector.

```rust
// In the runtime — illustrative
for desc in descriptors {
    match desc.integration {
        IntegrationKind::BitcoinCoreRpc { .. } => {
            let conn = node_registry.bitcoin_nodes.get(&btc_id).cloned()?;
            polling.push(Box::new(BitcoinCoreRpcCollector::new(desc, conn, http.clone())?));
        }
        IntegrationKind::BitcoinCoreZmq => {
            subscription.push(Box::new(BitcoinCoreZmqCollector::new(desc, conn, …)?));
        }
        // …etc
    }
}
```

Contributors adding a new collector add a new module + one match arm.
Plugin-style runtime registration (V1+) can be added without
disturbing this V0 shape.

### C3.3 — Module layout

```
src/collectors/
├── traits.rs                       # ADR-C1 traits + BatchSink
├── types.rs                        # existing
├── registry.rs                     # existing (NodeRegistry)
├── error.rs                        # existing (currently empty; can fold into types.rs)
├── bitcoin_core/
│   ├── mod.rs
│   ├── rpc.rs                      # BitcoinCoreRpcCollector + impl PollingCollector
│   ├── rpc_client.rs               # BitcoinRpcClient (thin reqwest wrapper)
│   └── zmq.rs                      # BitcoinCoreZmqCollector (V0.1+; stubbed in V0)
├── lnd/
│   ├── mod.rs
│   ├── grpc_poll.rs                # V0.1+
│   ├── grpc_stream.rs              # V0.1+
│   └── rest.rs                     # V0.1+
└── host/
    └── mod.rs                      # HostCollector (V0.1+)
```

### C3.4 — V0 RPC coverage for `BitcoinCoreRpcCollector`

Four RPCs per poll, in order:

| RPC | State produced | Catalog rules served |
|---|---|---|
| `getblockchaininfo` | `BitcoinBlockchainState`   | A1 tip lag, A2 IBD stall, IBD detection, verification progress |
| `getmempoolinfo`    | `BitcoinMempoolState`      | A4 mempool full, minrelayfee climb |
| `getnetworkinfo`    | `BitcoinNetworkState`      | A3 peer starvation (networkactive, connections) |
| `getpeerinfo`       | `BitcoinPeerSummaryState`  | A3 peer count, inbound/outbound split |

Per RPC success, the collector also emits a
`HealthCheckObservation { target: "bitcoin.rpc.<method>", status: Ok }`
so health diagnostics can detect partial RPC failures.

`getchaintips` (A5/A6 reorg detection), `getrpcinfo`, and per-peer
detail (B-class diagnostics) are deferred.

### C3.5 — No ping at construction

`BitcoinCoreRpcCollector::new` validates *shape* (URL parses, auth is
well-formed) but does not hit the network. A down node should appear
as `ProbeResult::Failed` on the first poll, not as a startup error.
This lets the sidecar boot even if its target is briefly offline.

### C3.6 — Shared `reqwest::Client` injected by the runtime

One `reqwest::Client` per sidecar process, passed to every
HTTP-using collector. Connection pooling matters. The runtime creates
it once at startup with appropriate timeouts and TLS settings.

### C3.7 — Per-RPC timeout, default 5 seconds, configurable

Each RPC call is wrapped in `tokio::time::timeout(timeout_per_rpc, …)`.
Timeouts surface as `CollectionErrorKind::Timeout` inside
`ProbeResult::Failed`. The collector's overall `poll` has a deterministic
upper bound (N RPCs × per_rpc_timeout).

Configurable via `BitcoinCoreRpcCollectorConfig` for slow hosts or
pruned-node initialization:

```rust
pub struct BitcoinCoreRpcCollectorConfig {
    pub timeout_per_rpc: Duration,    // default 5s
}
```

### C3.8 — `BitcoinRpcClient` as a thin in-crate hand-rolled wrapper

Not a published RPC crate (`bitcoincore-rpc-async`); a small
crate-internal module that knows the JSON-RPC envelope and a method
per RPC we use. ~150 LOC.

```rust
pub struct BitcoinRpcClient {
    url: String,
    auth: BitcoinRpcAuth,
    http: reqwest::Client,
    timeout: Duration,
}

impl BitcoinRpcClient {
    pub async fn get_blockchain_info(&self) -> Result<GetBlockchainInfoResponse, RpcError>;
    pub async fn get_mempool_info(&self)    -> Result<GetMempoolInfoResponse, RpcError>;
    pub async fn get_network_info(&self)    -> Result<GetNetworkInfoResponse, RpcError>;
    pub async fn get_peer_info(&self)       -> Result<GetPeerInfoResponse, RpcError>;
}

pub enum RpcError {
    Network(reqwest::Error),
    Timeout,
    HttpStatus(u16),
    BitcoindError { code: i32, message: String },
    Decode(serde_json::Error),
    Auth,
}
```

`RpcError → CollectionErrorKind` mapping at the collector boundary:

```text
Network        → Unreachable
Timeout        → Timeout
Auth           → AuthenticationFailed
HttpStatus     → ProtocolError
BitcoindError  → InvalidResponse
Decode         → DecodeError
```

V0.1+ can replace this with `bitcoincore-rpc-async` if the surface
grows substantially; hand-rolled is the smallest dependency footprint
for V0.

**Rationale.** Each item independent; the unifying theme is "smallest
shape that gets a real collector running and matches the catalog's
V0 diagnostic needs."

**Spec updates queued.**

- § 7.3 — replace "empty trait file" with the full trait + sketch.
- § 7.4 — annotate `CollectionContext` extension.
- § 7.5 — add `RpcError` and the mapping table.
- § 7.6 — note shared `reqwest::Client` requirement.
- § 7.7 — replace "concrete collectors: none" with the V0 `BitcoinCoreRpcCollector`.
- § 7.8 — refresh open questions; mark trait shape resolved, scheduler still open.
- § 21.3 / § 21.6 — refresh trait + collector inventory.

---

## ADR-S1 — Per-collector tasks + central consumer

**Date.** 2026-05-17.
**Status.** Accepted.
**Scope.** How collectors are scheduled and how observations flow
through the pipeline.

**Context.** With nine clusters of components designed (collectors,
observations, read models, diagnostics, engine, notifier, suppression,
kind registry, repositories), the runtime needs a concrete shape to
wire them together. Three architectures were considered:

- A single tick loop iterating all collectors.
- Per-collector tasks feeding a central consumer task via a channel.
- A cron-style dispatcher with rich scheduling primitives.

Several existing invariants pre-constrain the choice:

- `IntegrationKind` already encodes per-collector intervals
  (`BitcoinCoreRpc { interval }`, `Host { interval }`, …).
- `ReadModelStore::apply(&mut self)` is single-writer by design (ADR-R3 §2).
- `IncidentEngine::handle(&mut self)` is single-writer by design (ADR-L4 §L4.4).
- `Notifier::dispatch` is already async and can fan out concurrently.
- `SubscriptionCollector::run` is long-lived; each subscription wants
  its own task.

**Decision.**

Per-collector tasks emit `ObservationBatch`es to a bounded
`mpsc::channel`. A single consumer task drains the channel and runs
the pipeline:

```text
[Polling collector tasks]          ─┐
  tokio::time::interval driven      │
  send batch on each tick           │
                                    │   mpsc::channel<ObservationBatch>
[Subscription collector tasks]     ─┤   (bounded, capacity 1024)
  long-lived stream consumers       │
  send batches as data arrives      │
                                    │
[Consumer task] ───────────────────┘
  loop:
    batch = rx.recv().await
    observation_store.append(batch.observations).await
    for obs in batch.observations:
        read_models.apply(obs)?
    let subject = entity_ref_from(batch.collector.target);
    let ctx = DiagnosticContext { now, subject, …read_models };
    for rule in rules:
        drafts = rule.evaluate(ctx)?
    for draft in drafts:
        outcome = engine.handle(RecordSignal(draft), now)?
        if let Some(obs) = outcome.signal_observation:
            observation_store.append(obs).await
            read_models.apply(&obs)?
        if let Some(inc) = outcome.touched_incident:
            incident_repo.save(&inc).await        // write-through
        for ev in outcome.lifecycle_events:
            notifier.dispatch(&ev, &compose(&ev)).await
```

The **single-consumer** property is architecturally load-bearing:

- The `&mut self` apply on `ReadModelStore` works without locks because
  only the consumer task ever mutates the store.
- The `&mut self` handle on `IncidentEngine` works without locks for
  the same reason.
- The write-through-to-repo invariant (ADR-L4 §L4.4) is naturally
  serialized — no concurrent `repo.save()` calls.
- Backpressure is automatic: if the consumer falls behind, the bounded
  channel fills and collectors block on `send`. This makes
  overload *visible* (collectors stall) rather than *invisible*
  (memory grows silently).

Polling collectors run their own `tokio::time::interval`, ticking at
their `IntegrationKind::interval()`. Subscription collectors run their
`run(ctx, sink)` method as a long-lived task, pushing batches as data
arrives. Both feed the same channel; the consumer doesn't distinguish.

**Rationale.**

- **Single-writer property** comes free with the channel pattern; no
  manual locking or `Arc<RwLock<…>>` needed at V0 scale.
- **Per-collector intervals** are already first-class on
  `IntegrationKind`; one task per collector lets each timer be its own
  concern.
- **Subscription collectors** fit naturally — same channel, no
  special-casing in the consumer.
- **Bounded channel** makes backpressure observable.

**Alternatives considered.**

- **Single tick loop (Option A)** — rejected: forces a coarsest-common
  interval or per-collector counting logic; sequential collection means
  one slow collector blocks faster ones; subscription collectors don't
  fit.
- **Cron-style dispatcher (Option C)** — rejected: over-engineered for
  V0; `IntegrationKind` encodes intervals as `Duration`, not cron
  expressions; adds machinery without solving a current problem.
- **Multiple consumer tasks** (workers draining the channel in
  parallel) — rejected: breaks the single-writer property and reintroduces
  the locking the channel pattern avoided.

**Spec updates queued.**

- § 12 — replace the "no runtime" pseudocode with the channel
  architecture and consumer loop.
- § 15 — add `src/runtime/` to the module layout.
- § 21 — add `RuntimeDeps`, `RuntimeError`, `RuntimeConfig`,
  supervisor / consumer / bootstrap modules to the inventory.

---

## ADR-S2 — Per-batch rule evaluation against the batch's subject

**Date.** 2026-05-17.
**Status.** Accepted.
**Scope.** When diagnostic rules fire and what subject they evaluate
against.

**Context.** Three triggers were considered for rule evaluation:
per-batch, periodic timer, or hybrid with an `interested_in` filter.
The cost difference at V0 scale is negligible (handful of rules × few
batches per minute = trivial cycles); the clarity difference is real.

**Decision.**

Rules are evaluated **per `ObservationBatch`**, against a single
subject derived from the batch's collector target. The consumer task
runs all rules sequentially:

```rust
let subject = entity_ref_from(&batch.collector.target);
// CollectorTarget::BitcoinNode(id) → EntityRef::BitcoinNode(id)
// CollectorTarget::LndNode(id)     → EntityRef::LndNode(id)
// CollectorTarget::Host(id)        → EntityRef::Host(id)

let ctx = DiagnosticContext {
    now: Utc::now(),
    subject: &subject,
    state: &read_models, metrics: &read_models, health: &read_models,
    capabilities: &read_models, heartbeats: &read_models, signals: &read_models,
};

for rule in &rules {
    let drafts = rule.evaluate(ctx.clone())?;
    // drafts feed engine.handle(RecordSignal(draft))
}
```

**Convention:** rules that don't apply to a subject kind return
`Ok(vec![])`. V0 does not enforce this via a trait method — it stays a
convention to keep the trait surface minimal.

**Cross-subject rules** are still possible: a rule evaluated with
`subject = LndNode(alice)` can query `ctx.state.latest_state` for any
other `EntityRef` and produce a draft against the LND subject using
remote-subject state as evidence. The B6 watchtower-lag rule (LND state
+ Bitcoin state) is the canonical case. The trait surface already
permits this; only convention has to be respected.

A rule evaluation failure (panic, `Err`) is logged and skipped — a
buggy rule must not poison the rest of the cycle:

```rust
for rule in &rules {
    let drafts = match rule.evaluate(ctx.clone()) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(rule_id = rule.id(), error = ?e,
                          "diagnostic rule failed; skipping");
            continue;
        }
    };
    /* … */
}
```

**Future optimization (V0.1+):**
- A rule applicability cache by subject kind (skip rules that
  declared `applicable_subjects() -> Vec<EntitySubjectKind>`).
- A periodic re-evaluation timer for slow-changing rules
  (e.g. once per minute, for rules that span hours of history).
Both are additive layers — no V0 architectural cost.

**Rationale.**

- **One trigger** keeps incident latency bounded by collector interval +
  pipeline time. Operators can reason about responsiveness.
- **Per-batch subject** matches the batch's natural provenance: a
  BitcoinCoreRpc batch is "about" a Bitcoin node; rules evaluated in
  that context default to that subject.
- **Cross-subject capability** is preserved via the read-model trait
  surface — the architecture doesn't preclude future correlated rules.

**Alternatives considered.**

- **Periodic timer (Option B)** — rejected: introduces a second clock,
  makes incident latency harder to bound, no benefit at V0 scale.
- **Hybrid with `interested_in` filter (Option C)** — rejected as a V0
  add: it's a pure optimization on top of (A) and can be retrofitted
  later without touching the rule trait surface.

**Spec updates queued.**

- § 9 — note the per-batch trigger and the cross-subject convention.
- § 12 — annotate the consumer loop with the subject-derivation step.

---

## ADR-S3 — Runtime small calls

**Date.** 2026-05-17.
**Status.** Accepted.
**Scope.** Eight low-risk runtime implementation choices.

**Decision.**

### S3.1 — Tokio multi-thread runtime

`#[tokio::main]` with the default multi-thread worker pool. Tokio is
already a transitive dependency via `teloxide` and `reqwest`. Multi-thread
because subscription collectors and notification dispatch want
concurrent IO. Worker count: tokio default (number of cores). No special
configuration in V0.

### S3.2 — Bounded mpsc channel for observation pipeline

`tokio::sync::mpsc::channel::<ObservationBatch>(1024)`. 1024 is the
starting point; configurable via `RuntimeConfig::channel_capacity`.
Backpressure is automatic: a slow consumer makes collectors block on
`send`. Unbounded would risk OOM; smaller would over-couple slow ticks.

### S3.3 — Shutdown via broadcast signal + SIGINT/SIGTERM

`tokio::sync::broadcast::Sender<()>` is the shutdown signal. All
long-lived tasks subscribe and `tokio::select!` against it alongside
their work. `tokio::signal::ctrl_c()` (SIGINT) and
`tokio::signal::unix::signal(SignalKind::terminate())` (SIGTERM) both
trigger the broadcast.

```text
On signal:
  1. broadcast::send(())
  2. Collector tasks exit their loops; their tx clones drop.
  3. When all senders drop, the channel closes; consumer.recv() returns None.
  4. Consumer drains any final batches (channel closed).
  5. Final repo persistence (the engine has no buffered state past
     each handle() call, so nothing extra needed).
  6. tokio::time::timeout(30s) wraps the wait; force-exit on expiry.
```

### S3.4 — Collector supervision: respawn with exponential backoff

Each collector is spawned with `tokio::task::spawn`; the supervisor
holds the `JoinHandle`. If a handle resolves to:

- `Ok(_)` for a polling collector — unexpected; respawn with backoff.
  Polling collectors loop until shutdown; they should never return Ok
  spontaneously.
- `Err(JoinError)` — panic; log + respawn with backoff. Polling
  collectors are contract-bound not to panic (ADR-C2), so this is a bug
  to investigate, but the sidecar mustn't die.
- For `SubscriptionCollector::run` returning `Err(CollectionError)` —
  expected on connection death; respawn with backoff.

Backoff schedule: `10s, 30s, 60s, 300s` capped. Reset to 10s after a
successful 5-minute run.

### S3.5 — Rule registry: hand-wired Vec

```rust
// src/runtime/rules.rs
pub fn all() -> Vec<Box<dyn DiagnosticRule>> {
    vec![
        // V0 rules — populated as they land
        // Box::new(rules::bitcoin::TipLagRule::new()),
        // Box::new(rules::bitcoin::PeerStarvationRule::new()),
        // Box::new(rules::host::DiskExhaustionRule::new()),
    ]
}
```

No factory, no registry trait. V0.1+ may add config-driven
enabling/disabling per rule ID.

### S3.6 — Module layout

```text
src/runtime/
├── mod.rs              # pub fn run(deps) -> Result<…>; re-exports
├── supervisor.rs       # collector task supervision + shutdown
├── consumer.rs         # the central pipeline consumer
├── rules.rs            # rules::all()
├── bootstrap.rs        # build_polling_collectors / build_subscription_collectors
└── config.rs           # RuntimeConfig — channel capacity, timeouts
```

`main.rs` becomes a thin bootstrap.

### S3.7 — main.rs structure

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();

    let config = Config::load_from_args_and_env()?;            // (config ADR — TBD)
    let node_registry = NodeRegistry::from_config(&config)?;
    let kinds = KindRegistry::load(config.kinds_config.as_deref())?;
    let sidecar_id = SidecarId(uuid::Uuid::now_v7());

    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let observation_store: Arc<dyn ObservationStore>     = Arc::new(/* TBD */);
    let incident_repo:     Arc<dyn IncidentRepository>   = Arc::new(/* TBD */);
    let suppression_repo:  Arc<dyn SuppressionRepository> = Arc::new(/* V0.1+ */);

    let read_models = ReadModelStore::new(ReadModelStoreConfig::default());
    let open_incidents = incident_repo.load_open().await?;
    let engine = IncidentEngine::new(kinds, sidecar_id, open_incidents);

    let notifier = Notifier::new(
        config.notification_rules.clone(),
        WebhookSender::new(http.clone()),
        config.telegram.as_ref().map(|c| TelegramService::new(c.clone())),
        config.discord.as_ref().map(|c| DiscordService::new(c.clone())),
    );

    let polling = bootstrap::build_polling_collectors(&config.collectors, &node_registry, &http)?;
    let subscription = bootstrap::build_subscription_collectors(&config.collectors, &node_registry, &http)?;

    runtime::run(RuntimeDeps {
        sidecar_id,
        polling_collectors: polling,
        subscription_collectors: subscription,
        rules: rules::all(),
        read_models,
        engine,
        notifier,
        observation_store,
        incident_repo,
        config: config.runtime,
    }).await
}
```

### S3.8 — Per-batch `DiagnosticContext` construction

The consumer derives the subject from the batch's collector target and
builds a fresh `DiagnosticContext` per batch (`&read_models` is the
same store on every call — it's a single struct that implements all
six trait surfaces, per ADR-R1):

```rust
let subject: EntityRef = match &batch.collector.target {
    CollectorTarget::BitcoinNode(id) => EntityRef::BitcoinNode(id.clone()),
    CollectorTarget::LndNode(id)     => EntityRef::LndNode(id.clone()),
    CollectorTarget::Host(id)        => EntityRef::Host(id.clone()),
};

let ctx = DiagnosticContext {
    now: Utc::now(),
    subject: &subject,
    state:        &read_models,
    metrics:      &read_models,
    health:       &read_models,
    capabilities: &read_models,
    heartbeats:   &read_models,
    signals:      &read_models,
};
```

The context is `Clone`-cheap (just trait references). Rules borrow it
during evaluation.

**Rationale.** Each item is independent. The unifying theme is
"smallest shape that gets a V0 binary running with a clean evolution
path to V0.1+."

**Spec updates queued.**

- § 12 — full runtime sketch and `main.rs` structure.
- § 15 — `src/runtime/` module tree.
- § 21 — `RuntimeDeps`, `RuntimeConfig`, `RuntimeError`,
  supervisor / consumer / bootstrap / rules-module identifiers added to
  inventory.

---

## ADR-P1 — SQLite backend via sqlx for all three repositories

**Date.** 2026-05-17.
**Status.** Accepted.
**Scope.** The storage backend for `ObservationStore`,
`IncidentRepository`, and `SuppressionRepository` (V0.1+).

**Context.** Three repositories need concrete impls. The original
proposal was JSONL append-only files (simple, transparent), but
Bithound has a stated upgrade path: cloud support for notifications,
log structuring, multi-sidecar deployment, dashboards. A SQL-native
approach is dramatically more future-proof when query semantics start
to matter (time-windowed dashboards, cross-incident correlation,
operator UI joins, cloud sync replay).

JSONL would force a backend migration the moment those features
arrive. Doing SQLite at V0 means the trait surface is exercised
against real query semantics from day one, and the cloud path is just
"swap SQLite for Postgres at the sqlx layer."

**Decision.**

Use **SQLite via `sqlx`** for all three repositories. One database
file (`bithound.db` by default), three tables, hybrid schema
(indexed columns for hot fields + JSON column for full domain
serialization).

### Dependencies

```toml
sqlx = { version = "0.8", default-features = false,
         features = ["runtime-tokio", "tls-rustls", "sqlite",
                     "chrono", "uuid", "migrate"] }
```

Macros disabled — runtime-checked queries (`sqlx::query`,
`sqlx::query_as`) rather than compile-time `query!` macros. The
compile-time check requires a real DB during builds and adds friction
for contributors; runtime-checked queries are sufficient at V0 scale.

### Schema (V0 initial migration)

```sql
-- migrations/0001_initial.sql
CREATE TABLE observations (
    id              BLOB PRIMARY KEY,           -- UUIDv7 (16 bytes)
    observed_at     INTEGER NOT NULL,           -- unix nanos
    received_at     INTEGER,
    subject_kind    TEXT NOT NULL,              -- EntitySubjectKind discriminant
    subject_id      TEXT NOT NULL,
    sidecar_id      BLOB NOT NULL,
    collector_id    TEXT NOT NULL,
    integration     TEXT NOT NULL,              -- IntegrationKind discriminant
    instance_label  TEXT NOT NULL,
    origin          TEXT NOT NULL,              -- ObservationOrigin discriminant
    payload_kind    TEXT NOT NULL,              -- ten ObservationPayload variants
    payload_json    TEXT NOT NULL,              -- full payload (serde-typed)
    attributes_json TEXT NOT NULL               -- Attributes map
) STRICT;

CREATE INDEX idx_obs_observed_at  ON observations (observed_at DESC);
CREATE INDEX idx_obs_subject      ON observations (subject_kind, subject_id, observed_at DESC);
CREATE INDEX idx_obs_payload_kind ON observations (payload_kind, observed_at DESC);

CREATE TABLE incidents (
    id           BLOB PRIMARY KEY,
    fingerprint  TEXT NOT NULL,                 -- IncidentFingerprint::as_key()
    kind         TEXT NOT NULL,
    subject_kind TEXT NOT NULL,
    subject_id   TEXT NOT NULL,
    severity     TEXT NOT NULL,
    status       TEXT NOT NULL,
    opened_at    INTEGER NOT NULL,
    updated_at   INTEGER NOT NULL,
    resolved_at  INTEGER,
    incident_json TEXT NOT NULL                 -- full Incident snapshot
) STRICT;

CREATE INDEX idx_inc_fingerprint ON incidents (fingerprint);
CREATE INDEX idx_inc_status      ON incidents (status);
CREATE INDEX idx_inc_resolved_at ON incidents (resolved_at);

CREATE TABLE suppression_rules (
    id           BLOB PRIMARY KEY,
    fingerprint  TEXT NOT NULL,
    until        INTEGER,
    reason       TEXT NOT NULL,
    actor        TEXT NOT NULL,
    created_at   INTEGER NOT NULL
) STRICT;

CREATE INDEX idx_supp_fingerprint ON suppression_rules (fingerprint, until);
CREATE INDEX idx_supp_until       ON suppression_rules (until);
```

`STRICT` tables (SQLite 3.37+) enforce type rigor — no silent
text-to-int coercion.

### Hybrid columns + JSON payload

Hot fields (timestamps, subject, status, fingerprint, payload kind)
are indexed columns; the full domain object is serialized as JSON in
`*_json`. Queries hit indexes; schema stays flexible against
payload-type evolution. Adding a new `StateObservation` variant
(per ADR-R1 §R1.5) requires zero schema migrations — it just lands in
`payload_json`.

### Startup configuration

```rust
pub async fn open_pool(path: &Path) -> Result<SqlitePool, StoreError> {
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect(&format!("sqlite://{}?mode=rwc", path.display()))
        .await?;
    sqlx::query("PRAGMA journal_mode = WAL").execute(&pool).await?;
    sqlx::query("PRAGMA synchronous  = NORMAL").execute(&pool).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}
```

- **WAL mode** allows concurrent readers alongside the single writer.
- **`synchronous = NORMAL`** trades a small durability window (loss of
  last few transactions on power failure) for ~3-10× write speedup.
  Acceptable given the observation log is recoverable from collectors.
- **sqlx migrations** are baked in; `migrations/0001_initial.sql`
  ships with the binary.

### Migration path to Postgres (V0.2+ cloud)

sqlx abstracts both. The repository implementations target the
`Sqlite` flavor; lifting to `Any` or duplicating implementations for
`Postgres` is mechanical when cloud sync lands. The schema as
written is largely portable (`STRICT` is the only SQLite-ism;
Postgres has stricter types natively).

**Rationale.**

- **Cloud-ready from day one.** sqlx + SQL-native is the right
  foundation when notifications, log structuring, and multi-sidecar
  deployment are the stated upgrade path.
- **Query semantics are first-class.** Diagnostic rules can later run
  time-windowed queries (e.g. "rate of force-closes in the last
  hour") without infrastructure work.
- **One file, one backend.** Single `bithound.db` is as easy to back
  up and inspect as JSONL was; `sqlite3` CLI is universally
  available.
- **WAL + STRICT + sqlx migrations** gives us crash safety, schema
  rigor, and an evolution story without bespoke machinery.

**Alternatives considered.**

- **JSONL append-only** — rejected: cloud upgrade forces a migration,
  loses query semantics, undercuts the stated direction.
- **rusqlite directly** — rejected: sqlx's async API and
  Postgres-portable abstraction matter for the cloud path; rusqlite
  is sync-only and SQLite-only.
- **`sqlx::query!` compile-time macros** — rejected for V0:
  contributor friction (real DB at build time), runtime-checked
  queries are sufficient at this scale.
- **In-memory only for V0** — rejected: state across restarts is a
  V0 requirement (incident history, sidecar identity).

**Spec updates queued.**

- § 13 — replace "no storage" with the full SQLite design.
- § 15 — add `migrations/` and `src/storage/` to the module layout.
- § 21.5 — refresh storage abstractions inventory.

---

## ADR-P2 — Storage trait shapes and impl sketches

**Date.** 2026-05-17.
**Status.** Accepted.
**Scope.** `ObservationStore` trait (new), the three repository
implementations, error taxonomy, concurrency model, retention policy.

**Context.** ADR-P1 picked the backend. ADR-P2 nails down the trait
surfaces, the impl shape, and the operational concerns (concurrency,
durability, retention).

**Decision.**

### P2.1 — `ObservationStore` trait

V0 ships the full surface (append + iter); no V0.1 deferral.

```rust
// src/storage/traits.rs
#[async_trait]
pub trait ObservationStore: Send + Sync {
    /// Persist a batch of observations atomically.
    async fn append_many(&self, batch: &[Observation]) -> Result<(), StoreError>;

    /// Stream observations whose observed_at >= since, in ascending order.
    /// Used by V0.1+ read-model replay (per ADR-R3 §R3.5/S3.7).
    async fn iter_since(
        &self,
        since: DateTime<Utc>,
    ) -> Result<BoxStream<'_, Result<Observation, StoreError>>, StoreError>;
}

pub enum StoreError {
    Io(std::io::Error),
    Database(sqlx::Error),
    Serialization(serde_json::Error),
    Corruption(String),
    NotInitialized,
}
```

`append` (singular) is a default-impl shim around `append_many(&[obs])`
on the trait — keeps the surface minimal while supporting both call
styles.

### P2.2 — `IncidentRepository` shape (per ADR-L4 §L4.6, with SQLite impl)

The trait is unchanged from ADR-L4; the impl uses sqlx:

```rust
pub struct SqliteIncidentRepository { pool: SqlitePool }

#[async_trait]
impl IncidentRepository for SqliteIncidentRepository {
    async fn load_open(&self) -> Result<Vec<Incident>, RepoError> {
        // SELECT incident_json FROM incidents WHERE status != 'Resolved'
        // …deserialize each row's incident_json into Incident.
    }
    async fn save(&self, incident: &Incident) -> Result<(), RepoError> {
        // INSERT … ON CONFLICT (id) DO UPDATE SET … (UPSERT)
        // — atomic state replacement per IncidentId.
    }
}
```

`SuppressionRepository` (V0.1) follows the same pattern.

### P2.3 — Concurrency

**No manual interior mutability.** `sqlx::SqlitePool` is internally
`Arc`-shared and handles concurrent access. Methods take `&self`,
hold a `SqlitePool` (which is `Clone`), and call `sqlx::query(…)`.

SQLite-level: WAL mode allows concurrent readers alongside the
single writer (per ADR-S1, the consumer task is the sole writer).
Connection pool size is `max_connections = 8` — accommodates the
writer plus query traffic from `iter_since` and future dashboards.

This **replaces** the earlier-considered `tokio::sync::Mutex<File>`
pattern; sqlx makes it unnecessary.

### P2.4 — Durability

- `journal_mode = WAL` — write-ahead log, faster commits, concurrent reads.
- `synchronous = NORMAL` — fdatasync on transaction commit (not per
  page), small durability window (loss of last few transactions on
  hard crash), 3-10× faster than `FULL`. Acceptable because
  observations are recoverable from collectors on the next tick.
- Incidents and suppression rules use the same default; the
  transactional boundary per `save` is enough for the low frequency.

For higher durability later (V0.2+), expose `synchronous = FULL`
behind a config flag.

### P2.5 — Retention policy (replaces the JSONL "rotation" concept)

SQLite doesn't have file rotation. The equivalent is periodic
deletion of old rows + `VACUUM` for space reclamation. A
`retention::run(pool, config)` background task fires on a configurable
interval:

```rust
// src/storage/retention.rs
pub struct RetentionConfig {
    pub observations_max_age: Option<Duration>,    // None = no limit
    pub incidents_max_age:    Option<Duration>,
    pub suppressions_grace:   Option<Duration>,     // delete after `until` + grace
    pub vacuum_interval:      Duration,
}

pub async fn run(pool: SqlitePool, config: RetentionConfig,
                 mut shutdown: broadcast::Receiver<()>)
{
    let mut ticker = tokio::time::interval(config.vacuum_interval);
    loop {
        tokio::select! {
            _ = ticker.tick() => { sweep(&pool, &config).await; }
            _ = shutdown.recv() => break,
        }
    }
}

async fn sweep(pool: &SqlitePool, cfg: &RetentionConfig) {
    let now_ns = Utc::now().timestamp_nanos_opt().unwrap();

    if let Some(age) = cfg.observations_max_age {
        sqlx::query("DELETE FROM observations WHERE observed_at < ?")
            .bind(now_ns - age.as_nanos() as i64)
            .execute(pool).await.ok();
    }
    if let Some(age) = cfg.incidents_max_age {
        sqlx::query("DELETE FROM incidents \
                     WHERE resolved_at IS NOT NULL AND resolved_at < ?")
            .bind(now_ns - age.as_nanos() as i64)
            .execute(pool).await.ok();
    }
    if let Some(grace) = cfg.suppressions_grace {
        sqlx::query("DELETE FROM suppression_rules \
                     WHERE until IS NOT NULL AND until < ?")
            .bind(now_ns - grace.as_nanos() as i64)
            .execute(pool).await.ok();
    }
    sqlx::query("VACUUM").execute(pool).await.ok();
}
```

V0 defaults (in TOML, per ADR-X1):

```toml
[storage.retention]
observations_max_age_days = 30
incidents_max_age_days   = 365
suppressions_grace_days  = 90
vacuum_interval_hours    = 24
```

Setting any age to `0` disables retention for that table
(`None` in the config struct).

### P2.6 — Module layout

```text
migrations/
└── 0001_initial.sql                  # ADR-P1 schema, shipped with the binary

src/storage/
├── mod.rs                            # re-exports
├── traits.rs                         # ObservationStore + StoreError
├── retention.rs                      # P2.5 background task
├── sqlite/
│   ├── mod.rs                        # open_pool helper
│   ├── observation_store.rs          # SqliteObservationStore
│   ├── incident_repository.rs        # SqliteIncidentRepository
│   └── suppression_repository.rs     # SqliteSuppressionRepository (V0.1)
└── memory/                           # in-memory test impls
    ├── observation_store.rs
    └── incident_repository.rs
```

`IncidentRepository` trait stays in `src/incidents/repository.rs`
(per ADR-L4 §L4.6); `SuppressionRepository` stays in
`src/incidents/suppression.rs` (per ADR-L5 §L5.6). `src/storage/`
holds the new `ObservationStore` trait + the SQLite + in-memory
**impls** for all three.

### P2.7 — In-memory impls for tests

`MemoryObservationStore` and `MemoryIncidentRepository` — zero I/O,
no fsync, used by unit/integration tests. Free correctness check that
the trait surface is sufficient and the consumer task works against
any backend.

### P2.8 — Corruption handling

JSON deserialization failure on a stored row is logged as `tracing::warn!`
and the row is skipped. A `host.storage.corruption` event observation
is emitted (when the observation pipeline is established). The
database itself is treated as authoritative; we don't try to repair
corrupted rows automatically.

**Rationale.** Each item independent. The unifying themes: sqlx's
pool eliminates the manual concurrency machinery; WAL + NORMAL gets
crash safety without the slowness of FULL; retention replaces
rotation cleanly; trait surface is sufficient for V0 + V0.1 replay.

**Spec updates queued.**

- § 13 — full storage section (replace "no storage" note).
- § 15.b — add `src/storage/` and `migrations/` to target layout.
- § 21.5 — refresh inventory.

---

## ADR-X1 — Single `bithound.toml`, env-var overrides only for secrets

**Date.** 2026-05-17.
**Status.** Accepted.
**Scope.** Config file layout, schema, loading, validation. Secrets
handling (X2 was bundled here on ratification).

**Context.** V0 needs a way to declare monitored entities, collectors,
notification rules, storage paths, runtime knobs, and secrets. The
config layer connects everything the ADR chain has designed.

**Decision.**

### X1.1 — Single `bithound.toml`

One TOML file holds everything. The only externally-referenced file
is the optional `[incidents].kinds_config_path` (per ADR-L1) for
operator-contributed incident kinds.

Default config path resolution:

1. `--config <path>` CLI flag (if provided).
2. `./bithound.toml` (current working directory).
3. `/etc/bithound/bithound.toml` (system-wide).

Failure to find a config at any of these is a hard error.

V0.1+ may add `[notifications.include]` / `[collectors.include]` for
directory-style splits if operators request them.

### X1.2 — Env-var overrides for secrets only

Secrets are loaded **via environment variable reference only.** No
inline secrets, no file references. Field suffix `_env` carries the
env var name:

```toml
[[bitcoin_nodes]]
id = "btc-alice"
rpc_url = "http://127.0.0.1:8332"

[bitcoin_nodes.auth]
type = "user_pass"
user = "bithound"
password_env = "BITHOUND_BITCOIN_ALICE_PASSWORD"

# Or cookie-based (no secret needed — bitcoind manages the cookie file):
# type = "cookie_file"
# path = "/var/lib/bitcoind/.cookie"
```

The choice (env-only, no file-ref) keeps the operator surface small:
one mechanism, one mental model, well-understood in container
orchestration. File-ref support can be added later if a real use case
emerges (`password_file = …` next to `password_env = …`); for V0,
env-only is sufficient.

Inline secrets are a parse error: any field named `*_password`,
`*_token`, `*_secret` requires the `_env` suffix.

`SecretString` (from the `secrecy` crate, already in dependencies)
wraps the loaded value — no debug-print, no accidental logging.

### X1.3 — Full V0 config schema

```toml
[sidecar]
id_file = "/var/lib/bithound/sidecar_id"        # SidecarId persistence
log_level = "info"                              # tracing filter

[storage]
db_path = "/var/lib/bithound/bithound.db"

[storage.retention]
observations_max_age_days = 30
incidents_max_age_days    = 365
suppressions_grace_days   = 90
vacuum_interval_hours     = 24

[runtime]
channel_capacity = 1024                         # ADR-S3 §S3.2
shutdown_deadline_seconds = 30                  # ADR-S3 §S3.3

[incidents]
kinds_config_path = "/etc/bithound/custom_kinds.toml"   # optional, ADR-L1

# ---- monitored entities ----
[[bitcoin_nodes]]
id = "btc-alice"
rpc_url = "http://127.0.0.1:8332"
zmq_endpoint = "tcp://127.0.0.1:28332"          # optional, used by ZMQ collector (V0.1+)

[bitcoin_nodes.auth]
type = "user_pass"
user = "bithound"
password_env = "BITHOUND_BITCOIN_ALICE_PASSWORD"

# [[lnd_nodes]] — V0.1+
# [[hosts]]     — V0.1+

# ---- collectors ----
[[collectors]]
id = "btc-alice-rpc"
target = { type = "bitcoin_node", id = "btc-alice" }
integration = { type = "bitcoin_core_rpc", interval_seconds = 10 }
instance_label = "alice"
description = "Bitcoin Core RPC polling for alice"

# ---- notifications ----
[notifications.telegram]
bot_token_env = "BITHOUND_TELEGRAM_BOT_TOKEN"
parse_mode = "html"

[notifications.discord]
# defaults — per-rule webhook_env carries the secret URL

[notifications.webhook]
# defaults

[[notification_rules]]
id = "00000000-0000-7000-8000-000000000001"
name = "critical-to-telegram"
enabled = true
min_severity = "critical"
event_kinds = []                                # empty = all kinds

[notification_rules.target]
type = "telegram"
chat_id = -1001234567890
```

### X1.4 — CLI surface (clap-derive)

```rust
#[derive(clap::Parser)]
struct Cli {
    /// Path to bithound.toml. Falls back to ./bithound.toml then /etc/bithound/bithound.toml.
    #[arg(long, short)]
    config: Option<PathBuf>,

    /// Print the merged config and exit (secrets shown as `<redacted>`).
    #[arg(long)]
    check_config: bool,

    /// Print version and exit.
    #[arg(long)]
    version: bool,
}
```

V0 is small on CLI surface intentionally — config file is the
operator interface.

### X1.5 — Env-var override syntax (non-secret keys)

For testing and containerization, non-secret keys can be overridden
via `BITHOUND_<SECTION>__<KEY>` (double underscore separator —
sections may contain single underscores):

```sh
BITHOUND_STORAGE__DB_PATH=/tmp/test.db \
BITHOUND_RUNTIME__CHANNEL_CAPACITY=64 \
  bithound --config ./bithound.toml
```

Applied **after** TOML parse, **before** secrets resolution. Useful
for tests and one-off runs; production deployments typically rely on
the TOML file.

### X1.6 — SidecarId persistence

On startup:
1. If `sidecar.id_file` exists and parses as a UUIDv7 → use it.
2. Otherwise generate a new `Uuid::now_v7()`, write to the file,
   `sync_data()`, and use it.

Ensures sidecar identity is stable across restarts. Observation
provenance (`ObservationSource.sidecar_id`) stays consistent.

### X1.7 — Validation: fail loudly

`Config::load_from_args_and_env()` performs all validation upfront:

- TOML parses cleanly.
- Required fields present.
- Cross-references resolve (`collectors[].target.id` exists in
  `bitcoin_nodes` / `lnd_nodes` / `hosts`).
- Env vars referenced in `*_env` fields exist (don't read them yet —
  presence check only).
- Storage paths are writable (try creating parent dirs).

Any failure → `ConfigError` with a clear message, exit code 78
(EX_CONFIG). No silent fallbacks.

### X1.8 — Bootstrap order

```
1. Parse CLI args (clap).
2. Resolve config path (--config flag → cwd → /etc).
3. Read TOML file → serde parse to ConfigToml.
4. Apply BITHOUND_* env overrides (non-secret keys).
5. Validate shape + cross-references.
6. Resolve secrets (read *_env vars into SecretString).
7. Read or generate SidecarId.
8. Open SqlitePool (run migrations).
9. Build NodeRegistry, KindRegistry, CollectorDescriptors, NotificationRules.
10. Hand off to runtime::run().
```

Any failure in steps 1–9 exits with code 78 + one-line error.

### X1.9 — Module layout

```text
src/config/
├── mod.rs                    # Config, Config::load_from_args_and_env, ConfigError
├── sidecar.rs                # SidecarConfig
├── storage.rs                # StorageConfig, RetentionConfig
├── runtime.rs                # RuntimeConfig
├── targets.rs                # BitcoinNodeConfig, LndNodeConfig, HostConfig + AuthConfig
├── collectors.rs             # CollectorDescriptorConfig + integration parsing
├── notifications.rs          # NotificationRulesConfig + per-sink configs
├── secrets.rs                # Env-var resolution helpers
└── cli.rs                    # Cli struct (clap derive)
```

**Rationale.** Single file matches V0 scale. Env-only secrets matches
standard ops practice without bespoke file-ref machinery. Fail-loud
validation prevents the "sidecar started but doing nothing useful"
class of bugs. The bootstrap order is documented to make
contribution review tractable.

**Alternatives considered.**

- **Inline secrets in TOML** — rejected: bad for git, bad for secrets
  management.
- **File-ref secrets (`_file` in addition to `_env`)** — deferred:
  env-only is sufficient for V0; adding `_file` later is mechanical.
- **Split config (`bithound.toml` + `notifications.toml` + etc.)** —
  deferred to V0.1+ if operators ask.
- **Compile-time `query!` macros** (in ADR-P1) had its own discussion;
  not duplicated here.

**Spec updates queued.**

- New § 13.bis or extend § 13 with config schema.
- § 15.b — `src/config/` and `migrations/` modules added.
- § 21 — add `Config`, `ConfigError`, retention types to inventory.

---

## ADR-D1 — Unvalidated vs validated incident signal draft

**Date.** 2026-05-18.
**Status.** Accepted.
**Scope.** Splitting `IncidentSignalDraft` into two types so the compiler
enforces that `KindRegistry::validate` has been called before the engine
acts on a draft. Aligns with Wlaschin's "Domain Modeling Made Functional"
pattern of making validation state visible in the type system.

**Context.** ADR-L1 §§1–2 introduced `IncidentSignalDraft` with `kind`
and `dimension` fields. `KindRegistry::validate_draft` checks them
against the registered specs. But the same type is used both before and
after validation, so nothing prevents the engine from acting on an
unchecked draft.

**Decision.**

### Two distinct structs

```rust
// src/diagnostics/types.rs
pub struct UnvalidatedIncidentSignalDraft {
    pub subject: EntityRef,
    pub signal: SignalName,
    pub kind: IncidentKind,
    pub dimension: Option<String>,
    pub severity: SignalSeverity,
    pub status: SignalStatus,
    pub confidence: Confidence,
    pub evidence: Vec<EvidenceRef>,
}
```

```rust
// src/incidents/kinds.rs (or src/incidents/types.rs)
pub struct ValidatedIncidentSignalDraft {
    subject: EntityRef,          // ← private
    signal: SignalName,
    kind: IncidentKind,
    dimension: Option<String>,
    severity: SignalSeverity,
    status: SignalStatus,
    confidence: Confidence,
    evidence: Vec<EvidenceRef>,
}

impl ValidatedIncidentSignalDraft {
    pub fn subject(&self) -> &EntityRef { &self.subject }
    pub fn kind(&self) -> &IncidentKind { &self.kind }
    pub fn dimension(&self) -> Option<&str> { self.dimension.as_deref() }
    pub fn severity(&self) -> &SignalSeverity { &self.severity }
    pub fn status(&self) -> &SignalStatus { &self.status }
    pub fn confidence(&self) -> &Confidence { &self.confidence }
    pub fn evidence(&self) -> &[EvidenceRef] { &self.evidence }
    pub fn signal(&self) -> &SignalName { &self.signal }
}

impl KindRegistry {
    pub fn validate(
        &self,
        draft: UnvalidatedIncidentSignalDraft,
    ) -> Result<ValidatedIncidentSignalDraft, DraftError>;
}
```

The unvalidated form has public fields — rules construct it directly.
The validated form has **private fields and accessor methods**; the
only way to construct one is via `KindRegistry::validate`. The
compiler enforces the gate.

### "Unvalidated" represents trust state, not origin

A draft replayed from the observation log, imported from a backup, or
constructed in a test is still `Unvalidated` until checked. The kind
registry may have changed between original emission and re-validation.

### Serialization asymmetry

- `UnvalidatedIncidentSignalDraft` is `Clone + Debug + Serialize + Deserialize`.
- `ValidatedIncidentSignalDraft` is `Clone + Debug + Serialize` but
  **not** `Deserialize`. Drafts deserialized from storage come back as
  `Unvalidated` and must re-validate. Stale validation results cannot
  re-enter the engine.

### Engine input

`IncidentCommand::RecordSignal` carries an `UnvalidatedIncidentSignalDraft`.
The engine's `handle` validates as the first step.

```rust
impl IncidentEngine {
    pub fn handle(&mut self, cmd: IncidentCommand, now: DateTime<Utc>)
        -> Result<Vec<IncidentEvent>, EngineError>
    {
        match cmd {
            IncidentCommand::RecordSignal(unvalidated) => {
                let validated = self.kinds.validate(unvalidated)
                    .map_err(EngineError::Draft)?;
                self.lift_signal(validated, now)
            }
            // … other variants (see ADR-D3)
        }
    }
}
```

**Rationale.**

- **Compile-time enforcement** of the validation gate. No "did we
  remember to validate?" bugs possible.
- **Two named structs** is simpler than `IncidentSignalDraft<State>`
  phantom-type markers in Rust — derive macros work cleanly, error
  messages are readable, no marker types to remember.
- **Trust-state framing** (not origin-based) correctly handles replay,
  imports, and tests with one rule: anything not produced by
  `KindRegistry::validate` is `Unvalidated`.

**Alternatives considered.**

- **Type-state via PhantomData** (`IncidentSignalDraft<Validated>`):
  rejected — ceremonious Debug/Serialize, worse error messages, no
  upside over two named structs.
- **Newtype wrapper** (`ValidatedDraft(IncidentSignalDraft)`): rejected
  — no symmetric way to express the unvalidated state.
- **Runtime `validated: bool` flag**: rejected — runtime checks are
  exactly what this ADR moves away from.

**Spec updates queued.**

- § 9.2 — `DiagnosticRule::evaluate` returns
  `Vec<UnvalidatedIncidentSignalDraft>`.
- § 10.5 — engine accepts `ValidatedIncidentSignalDraft` internally;
  validation is the first step of `handle`.
- § 21.2 — add both struct names to the inventory.

---

## ADR-D2 — Smart constructors for name newtypes

**Date.** 2026-05-18.
**Status.** Accepted.
**Scope.** Adding parse-or-fail constructors to the ten dotted-namespace
name newtypes (`IncidentKind`, `MetricName`, `SignalName`, `StateName`,
`HealthTargetId`, `CapabilityName`, `EventName`, `TransitionName`,
`InventoryName`, `DiagnosisName`).

**Context.** The name newtypes are currently
`pub struct X(pub String)` — any string, including the empty string,
control characters, or malformed dot-separated forms, can be wrapped.
Validation happens elsewhere (registry checks, well_known parity tests).
DMMF says validation belongs at construction; the type guarantees the
invariant.

**Decision.**

### Shared validation rule

All ten newtypes share the same parse rule:

- Two or more dot-separated segments.
- Each segment matches `[a-z][a-z0-9_]*`.
- Total length: 1–128 characters.
- ASCII printable (the regex above enforces it).

Reference regex (documentation only — actual parser is hand-written for
better error messages):

```
^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+$
```

Valid: `bitcoin.tip_lag`, `lnd.channel.inactive`, `host.disk.exhaustion`,
`sidecar.collector.run_started`.
Invalid: `tip_lag` (no dot), `BitcoinTipLag` (uppercase),
`bitcoin..tip_lag` (empty segment), `1bitcoin.x` (digit start),
`bitcoin.tip-lag` (hyphen).

### Shared parser

```rust
// src/shared/parse.rs
pub fn parse_dotted_name(s: &str) -> Result<String, ParseDottedNameError>;

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ParseDottedNameError {
    #[error("name is empty")] Empty,
    #[error("name exceeds 128 characters (got {got})")] TooLong { got: usize },
    #[error("invalid character {found:?} at position {at}")]
    BadCharacter { at: usize, found: char },
    #[error("empty segment at position {at}")] EmptySegment { at: usize },
    #[error("segment at position {at} must start with a-z")] BadSegmentStart { at: usize },
    #[error("name must contain at least one dot")] NoDot,
}
```

### Private inner field, parse-or-fail constructor

Each newtype becomes (template):

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct IncidentKind(String);

impl IncidentKind {
    pub fn parse(s: impl AsRef<str>) -> Result<Self, ParseDottedNameError> {
        parse_dotted_name(s.as_ref()).map(Self)
    }

    pub fn as_str(&self) -> &str { &self.0 }
}

impl AsRef<str> for IncidentKind { fn as_ref(&self) -> &str { &self.0 } }
impl std::fmt::Display for IncidentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for IncidentKind {
    type Error = ParseDottedNameError;
    fn try_from(s: String) -> Result<Self, Self::Error> { Self::parse(s) }
}
impl From<IncidentKind> for String {
    fn from(k: IncidentKind) -> String { k.0 }
}
```

`Serialize` and `Deserialize` round-trip through the string; the
`try_from = "String"` attribute makes **deserialization re-validate**.
A maliciously-crafted JSON or TOML value with an invalid name fails to
deserialize.

### Fast path for well-known constants

```rust
impl IncidentKind {
    /// Construct from a known-valid `&'static str`. Debug-asserts the
    /// parse rule; the `well_known::*` constants are themselves validated
    /// by parity unit tests, so release builds skip the check.
    pub fn from_well_known(s: &'static str) -> Self {
        debug_assert!(parse_dotted_name(s).is_ok(), "invalid well_known name: {s}");
        Self(s.to_string())
    }
}
```

A unit test in each `well_known.rs` calls `parse_dotted_name` on every
constant; an invalid constant fails the build before release.

### Per-name semantic helpers

`StateObservation::name()` (BTH-6 — already shipped) returns a
`StateName` constructed via `from_well_known`. Other typed observation
helpers (`MetricObservation::name`, etc.) get the same treatment as
their tickets land.

### Migration plan

The migration is mechanical but scattered. Tickets (see § 24
implementation plan for D-cluster):

- **BTH-D2.a** — Add `src/shared/parse.rs` with `parse_dotted_name` and
  `ParseDottedNameError`. Compatibility helpers (`From<String>` parallel
  to existing `pub` field) so subsequent tickets don't break the build.
- **BTH-D2.b** — Migrate `IncidentKind` to private field + `parse`.
  Update all call sites (well_known references, tests).
- **BTH-D2.c** — Migrate `MetricName` and `SignalName`.
- **BTH-D2.d** — Migrate `StateName` and `CapabilityName`.
- **BTH-D2.e** — Migrate the remaining names (`HealthTargetId`,
  `EventName`, `TransitionName`, `InventoryName`, `DiagnosisName`).
  Remove the compatibility helpers.

**Rationale.**

- **Make illegal states unrepresentable** at the type system level
  (Wlaschin's central tenet).
- **Single parse rule** for all ten newtypes — they share the same
  semantic shape; one parser, one error type, one test suite.
- **Private inner field with `as_str()` accessor** preserves serde
  round-tripping and string display while gating construction.
- **`try_from = "String"` attribute** is what makes deserialization
  re-validate; the serde-default `Deserialize` would bypass `parse`.
- **Well-known fast path** preserves V0 ergonomics for the constants
  that ship with the binary.

**Alternatives considered.**

- **Keep `pub` inner fields, add `parse()` as advisory**: rejected
  — the field bypass defeats the gate.
- **Per-newtype parse rules**: rejected — wasteful, all ten share
  semantic shape.
- **Single `Name(String)` newtype across all ten**: rejected — loses
  type-level distinction between `IncidentKind` and `MetricName`.
- **Stricter regex** (e.g., max 3 segments, max 32 chars per segment):
  rejected — the catalog has 4-segment names like
  `sidecar.collector.run_started`; arbitrary caps invite future churn.

**Spec updates queued.**

- § 21.2 — annotate name newtypes as smart-constructed.
- § 21.5 (or new § 21.bis) — add `parse_dotted_name` and
  `ParseDottedNameError` to a "shared utilities" inventory.
- ADR-L1 §5 — annotate `well_known::*` constants as paired with the
  per-newtype `from_well_known()` fast path.

---

## ADR-D3 — Full command vocabulary (Incident + Suppression services)

**Date.** 2026-05-18.
**Status.** Accepted (commands defined now; V0.2 handlers return
`NotYetImplemented`).
**Scope.** Define the complete command vocabulary for both the incident
engine and the (separate) suppression service. Stub V0.2-only handlers
that aren't wired in V0/V0.1.

**Context.** ADR-L4 §L4.3 defined only `IncidentCommand::RecordSignal`
for V0; `Acknowledge`, `Suppress`, `Unsuppress`, `Resolve` were deferred
to V0.2 alongside the operator UI. Leaving the command surface incomplete
means callers can't know what V0.2 will support without consulting
documentation. DMMF says: the command vocabulary should be complete at
the type level even if handlers are stubs.

**Decision.**

### Two distinct command enums

Suppression is notifier-side per ADR-L5 §L5.2 and acts on a different
aggregate (the `SuppressionRule` registry, not the `Incident`).
Mixing them on one enum would couple the engine to suppression policy.

```rust
// src/incidents/engine.rs
pub enum IncidentCommand {
    RecordSignal(UnvalidatedIncidentSignalDraft),
    Acknowledge {
        id: IncidentId,
        by: ActorId,
        at: DateTime<Utc>,
    },
    Resolve {
        id: IncidentId,
        by: ActorId,
        at: DateTime<Utc>,
        reason: String,
    },
}

// src/incidents/suppression.rs
pub enum SuppressionCommand {
    Suppress {
        fingerprint: IncidentFingerprint,
        until: Option<DateTime<Utc>>,
        by: ActorId,
        reason: String,
    },
    Unsuppress {
        fingerprint: IncidentFingerprint,
        by: ActorId,
    },
}
```

### Stub behavior: return `NotYetImplemented`

```rust
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("draft validation: {0:?}")] Draft(DraftError),
    #[error("command not yet implemented: {0}")] NotYetImplemented(&'static str),
}

impl IncidentEngine {
    pub fn handle(&mut self, cmd: IncidentCommand, now: DateTime<Utc>)
        -> Result<Vec<IncidentEvent>, EngineError>
    {
        match cmd {
            IncidentCommand::RecordSignal(draft) => self.handle_record_signal(draft, now),
            IncidentCommand::Acknowledge { .. } =>
                Err(EngineError::NotYetImplemented("Acknowledge")),
            IncidentCommand::Resolve { .. } =>
                Err(EngineError::NotYetImplemented("Resolve")),
        }
    }
}
```

Stubs return `Err(EngineError::NotYetImplemented(name))` rather than
`todo!()` because:

- No runtime panics on misrouted commands.
- The future operator UI can ship commands incrementally, gating each
  on the engine's implementation status.
- Tests can verify the error message rather than catching panics.

### `ActorId` promotion

`ActorId` (strawmaned in ADR-L5 §L5.5) is now referenced by both
`IncidentCommand` and `SuppressionCommand`. Promote to
`src/shared/types.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActorId(pub String);

impl ActorId {
    pub fn system() -> Self { Self("system".into()) }
    pub fn operator(name: impl Into<String>) -> Self { Self(name.into()) }
}
```

Field stays `pub` for V0/V0.1; V0.2's operator UI work introduces real
user identity, RBAC, and audit. The named constructors document intent
in the meantime.

### `SuppressionService` trait

```rust
// src/incidents/suppression.rs
#[async_trait]
pub trait SuppressionService: Send + Sync {
    async fn handle(&self, cmd: SuppressionCommand, now: DateTime<Utc>)
        -> Result<(), SuppressionError>;
}

#[derive(Debug, thiserror::Error)]
pub enum SuppressionError {
    #[error("not yet implemented: {0}")] NotYetImplemented(&'static str),
    #[error("repository: {0}")] Repository(RepoError),
}
```

V0/V0.1 has no concrete `SuppressionService`. The trait is defined so
V0.2 can drop a concrete impl in without architectural change.

### Exhaustive match enforced

The engine's `handle` is an exhaustive match. Adding a new
`IncidentCommand` variant is a compile error until a handler arm
exists. Catches "added a command, forgot to wire it" mistakes.

**Rationale.**

- **Type-level completeness** of the command surface lets downstream
  code (UI, CLI, REST API) discover what's coming via the type system.
- **Engine vs suppression split** matches ADR-L5's notifier-side
  suppression model.
- **Graceful unhandled commands** via `NotYetImplemented` keep the
  binary robust under misrouting.
- **`ActorId` in `src/shared/types.rs`** because two command enums use
  it.

**Alternatives considered.**

- **One `IncidentCommand` enum handling everything**: rejected —
  couples engine to suppression policy.
- **`todo!()` stubs**: rejected — runtime panics in production are
  unacceptable.
- **`#[cfg(feature = "operator-ui")]` gating**: rejected — complicates
  the type model and creates compile-time matrix problems.

**Spec updates queued.**

- § 10.5 — engine command set: RecordSignal + Acknowledge + Resolve.
- § 10.7 — `SuppressionCommand` and `SuppressionService` trait.
- § 21.2 — promote `ActorId` from "designed (V0.1)" to "shared types".
- § 21.4 — both command enums in the inventory.

---

## ADR-D4 — Cross-context domain events (β: events-only output)

**Date.** 2026-05-18.
**Status.** Accepted. **Supersedes ADR-L4 §L4.2 (`HandleOutcome`).**
**Scope.** Workflow output shape. Specifically: the engine and other
workflows return `Vec<Event>` rather than structured outcome objects.

**Context.** ADR-L4 §L4.2 defined `HandleOutcome` as the engine's
output — a struct with `signal_observation`, `touched_incident`, and
`lifecycle_events` fields. The shape was chosen for static `Option<…>`
invariants (at-most-one signal observation per command).

Two factors changed the calculus during ADR-D4 review:

1. **Fleet management is on the explicit roadmap.** During the D4
   brainstorm the user confirmed cloud-side fleet management will be
   built ("within ~12 months"). This means events must leave the
   sidecar process at some point — they go to a cloud control plane for
   aggregation, dashboards, and centralized alerting.
2. **Cross-process consumers need a uniform event surface.** A sidecar
   pushing events to the cloud, exposing events to a future operator
   UI, or feeding peer sidecars all want the same shape: a stream of
   immutable, typed, named events. Deriving that stream from
   `HandleOutcome` works (the α+ alternative considered during
   brainstorm) but creates a drift-risk surface between two parallel
   representations.

Going β (events-only) now means the engine's behavior **is** the event
stream — no separate derivation, no drift, no migration when cloud sync
lands.

**Decision.**

### Engine returns `Vec<IncidentEvent>`

```rust
impl IncidentEngine {
    pub fn handle(&mut self, cmd: IncidentCommand, now: DateTime<Utc>)
        -> Result<Vec<IncidentEvent>, EngineError>;
}
```

`HandleOutcome` is removed. All engine state changes are emitted as
events. **This supersedes ADR-L4 §L4.2.**

### Event hierarchy

```rust
// src/incidents/events.rs
pub enum IncidentEvent {
    /// Signal observation produced by the engine. Caller persists to
    /// observation store and applies to read models.
    SignalRecorded(Observation),

    /// Draft was validated and produced an incident state change.
    /// Caller persists to incident repository. Carries the full
    /// incident object — never delta fields — so the event is
    /// self-contained for cloud sync.
    IncidentTouched(Incident),

    /// Notify-worthy lifecycle transition. Caller dispatches via
    /// notifier. Wraps the existing `IncidentLifecycleEvent` so
    /// notification code is unchanged.
    Lifecycle(IncidentLifecycleEvent),

    /// Validation rejected the draft. No state change occurred.
    /// Audit-loggable; not dispatched to notifier.
    DraftRejected {
        rule_id: String,
        error: DraftError,
    },

    /// Draft was below the kind's `min_open_confidence` floor.
    /// `SignalRecorded` was already emitted; no incident lift.
    DraftBelowConfidenceFloor {
        kind: IncidentKind,
        confidence: Confidence,
        floor: Confidence,
    },
}
```

### Per-context events modules

```rust
// src/observations/events.rs
pub enum ObservationEvent {
    BatchProduced(ObservationBatch),
    ObservationAppended { id: ObservationId, payload_kind: &'static str },
}

// src/read_models/events.rs
pub enum ReadModelEvent {
    Applied { observation_id: ObservationId, projection: &'static str },
}

// src/diagnostics/events.rs
pub enum DiagnosticEvent {
    DraftEmitted { rule_id: String, draft: UnvalidatedIncidentSignalDraft },
    RuleFailed { rule_id: String, error: String },
}

// src/notifications/events.rs
pub enum NotificationEvent {
    Dispatched {
        rule_id: NotificationRuleId,
        receipt: DeliveryReceipt,
    },
    Suppressed {
        rule_id: NotificationRuleId,
        suppression_rule: SuppressionRuleId,
    },
}
```

### Top-level `DomainEvent` envelope

```rust
// src/domain_events.rs
pub enum DomainEvent {
    Observation(ObservationEvent),
    ReadModel(ReadModelEvent),
    Diagnostic(DiagnosticEvent),
    Incident(IncidentEvent),
    Notification(NotificationEvent),
}
```

Used for cross-context audit logging, cloud push, and any future event
bus. Each context owns its own events module; the top-level envelope
sums them.

### Caller dispatch pattern

```rust
let events = engine.handle(cmd, now)?;
for event in events {
    match event {
        IncidentEvent::SignalRecorded(observation) => {
            observation_store.append(&observation).await?;
            read_models.apply(&observation)?;
        }
        IncidentEvent::IncidentTouched(incident) => {
            incident_repo.save(&incident).await?;
        }
        IncidentEvent::Lifecycle(lifecycle_event) => {
            let msg = compose_notification_message(&lifecycle_event);
            notifier.dispatch(&lifecycle_event, &msg).await;
        }
        IncidentEvent::DraftRejected { rule_id, error } => {
            tracing::warn!(rule_id, error = ?error, "diagnostic draft rejected");
        }
        IncidentEvent::DraftBelowConfidenceFloor { kind, confidence, floor } => {
            tracing::debug!(?kind, ?confidence, ?floor, "draft below confidence floor");
        }
    }
}
```

### Ordering invariant

Within a single `handle()` call, events are emitted in the order side
effects must occur:

1. **`SignalRecorded(_)`** first — persistence and read-model update
   precede any incident reasoning.
2. **`IncidentTouched(_)`** next — incident state durable before
   notification.
3. **`Lifecycle(_)`** last — notify only after the incident is
   persisted.
4. **`DraftRejected` / `DraftBelowConfidenceFloor`** can appear
   anywhere — they're terminal-or-no-op outcomes.

This ordering is the **engine's responsibility**. Runtime code iterates
events sequentially, trusting the order. A unit-tested invariant.

### Cloud-sync handoff

The same `Vec<IncidentEvent>` (or its `DomainEvent` envelope) is what
gets pushed to the cloud control plane when fleet management lands.
No conversion step, no derivation — the engine's local behavior and
the cloud's view are the same data structure.

**Recovered invariants.**

`HandleOutcome` encoded two invariants via `Option<…>`: "at most one
signal observation per command" and "at most one touched incident."
With `Vec<IncidentEvent>` those are runtime invariants enforced by:

- The engine implementation (each command's handler emits at most one
  `SignalRecorded` and at most one `IncidentTouched`).
- Unit tests that assert event counts per command shape.

Tradeoff accepted: we lose compile-time enforcement of these
multiplicities in exchange for the cloud-readiness benefits.

**Rationale.**

- **Cloud fleet management is committed roadmap** (~12 months). β
  eliminates the drift surface and the future refactor cost.
- **One source of truth.** The engine's behavior is the event stream.
- **Pay the refactor cost once.** α+ now → β later is 2× work; β now
  is 1× work plus a slightly steeper initial learning curve.
- **Whole domain objects in events** (full `Incident`, not delta
  fields) means cloud consumers don't need engine state to interpret
  events.

**Alternatives considered.**

- **α (naming only)**: rejected — events would be dead code, prone to
  drift.
- **α+ (events derived from `HandleOutcome` for tracing)**: rejected
  once cloud was confirmed — drift risk + refactor cost.
- **Hybrid (`HandleOutcome` plus events for cloud only)**: rejected as
  worst-of-both.

**Migration impact (D4 supersedes pieces of L4/L2/S3).**

- **ADR-L4 §L4.2** (`HandleOutcome` shape): **superseded**.
- **ADR-L2's decision diagram** (§ "Putting it together"): updated to
  emit events instead of outcome fields.
- **ADR-S3 §S3.8** (per-batch consumer pattern): updated to
  pattern-match events.
- **BTH-17** ticket (`HandleOutcome`, `EngineError`): re-scoped to
  define `IncidentEvent` and `EngineError`.
- **BTH-19** ticket (engine `handle` decision tree): re-scoped to
  return `Vec<IncidentEvent>`.
- **BTH-35** ticket (consumer module): re-scoped to event dispatch.

**Spec updates queued.**

- § 10.5 — engine returns `Vec<IncidentEvent>`; `HandleOutcome` removed.
- § 10.5.1 — receive-a-draft flow updated.
- § 10.5.2 — caller responsibilities updated.
- § 12.1 — consumer loop pattern updated.
- § 21.4 — replace `HandleOutcome` with `IncidentEvent` and per-context
  events in the inventory.
- ADR-L4 §L4.2 — annotated as superseded.

---

## ADR-P3 — Notification attempts persistence (with durable retry)

**Date.** 2026-05-19.
**Status.** Accepted.
**Scope.** Persistent storage of notification dispatch attempts; durable
retry queue across restarts; state machine for in-flight, succeeded,
failed-transient, failed-permanent, and suppressed deliveries.

**Context.** ADR-L5 declared the `DeliveryOutcome` taxonomy (Delivered,
Transient, Permanent, Suppressed) and noted that suppressed deliveries
should leave an audit trail. ADR-P1 and ADR-P2 designed the storage
layer but only covered observations, incidents, and (in V0.1)
suppression rules — notification attempts were not addressed.

In code today, `NotificationAttempt` and `DeliveryReceipt` types exist
but are unused. `Notifier::dispatch` returns
`Vec<(NotificationRuleId, DeliveryReceipt)>` and the receipts are
dropped after the call. This is a real gap: operators have no
notification audit log; transient failures silently vanish; suppressed
deliveries aren't recorded; cloud sync (per ADR-D4 motivation) has
nothing to push for the notification side of the pipeline.

The original brainstorm proposed completed-only persistence for V0 with
durable retry deferred to V0.2. After review, the user committed to
durable retry from V0 — the additional complexity is bounded at this
scale and avoids a future refactor.

**Decision.**

### P3.1 — State machine and per-row immutability

Each delivery attempt is a single row in `notification_attempts`. Rows
move from `Pending` to exactly one terminal state and stay there:

```text
[Pending]
  ├──(Delivered)─────────→ [Succeeded]         (terminal)
  ├──(Permanent error)──→ [FailedPermanent]   (terminal)
  ├──(Suppressed)────────→ [Suppressed]       (terminal — ADR-L5)
  └──(Transient error)──→ [FailedTransient]   (terminal-for-this-row)
                              │
                              └──(scheduler retries)──→ NEW ROW
                                                        attempt_number + 1
                                                        parent_attempt_id = original.id
                                                        status = Pending
```

**Retries do not mutate prior rows.** Each retry inserts a new row with
`attempt_number` incremented and `parent_attempt_id` pointing back. The
chain (followed via `parent_attempt_id`) is the full retry history per
logical delivery.

Each row has exactly one INSERT (when status is `Pending`) and one
UPDATE (when transitioning to a terminal status). No row ever
transitions between terminal states.

### P3.2 — Schema

```sql
CREATE TABLE notification_attempts (
    id                BLOB PRIMARY KEY,        -- NotificationAttemptId (UUIDv7)
    rule_id           BLOB NOT NULL,            -- NotificationRuleId
    incident_id       BLOB NOT NULL,            -- joins to incidents.id
    lifecycle_kind    TEXT NOT NULL,            -- 'Opened' | 'Escalated' | 'Resolved'

    target_kind       TEXT NOT NULL,            -- 'telegram' | 'discord' | 'webhook' | 'stdout'
    target_summary    TEXT NOT NULL,            -- redacted target description

    status            TEXT NOT NULL,            -- 'Pending' | 'Succeeded'
                                                --   | 'FailedTransient' | 'FailedPermanent'
                                                --   | 'Suppressed'
    attempt_number    INTEGER NOT NULL,         -- 1 on initial, increments per retry
    parent_attempt_id BLOB,                     -- NULL for first attempt

    next_retry_at     INTEGER,                  -- unix nanos; set on FailedTransient
                                                --   iff retries remain

    outcome_kind      TEXT,                     -- NULL while Pending; one of
                                                --   'Delivered' | 'Transient' | 'Permanent'
                                                --   | 'Suppressed'
    outcome_json      TEXT,                     -- full DeliveryOutcome serialized
    external_ref_json TEXT,                     -- ExternalMessageRef JSON if Delivered with one

    attempted_at      INTEGER NOT NULL,         -- unix nanos at INSERT (status=Pending)
    completed_at      INTEGER                   -- unix nanos at terminal UPDATE
) STRICT;

CREATE INDEX idx_attempts_incident_id        ON notification_attempts (incident_id);
CREATE INDEX idx_attempts_rule_id            ON notification_attempts (rule_id);
CREATE INDEX idx_attempts_status_next_retry  ON notification_attempts (status, next_retry_at);
CREATE INDEX idx_attempts_attempted_at       ON notification_attempts (attempted_at DESC);
```

Design notes:

- **No FK on `incident_id`** — same hybrid-column philosophy as ADR-P1.
  Avoids cascade-delete surprises during retention sweeps.
- **`target_summary` is the only target column.** Full targets carry
  `SecretString` URLs and tokens; those are *never* persisted. The
  summary is human-readable redacted form:
  `telegram:chat_id=-1001234`, `discord:webhook=host=hooks.discord.com`,
  `webhook:host=ops.example.com`. The actual secret is reconstructed
  from config at dispatch time.
- **`outcome_json`** carries the full `DeliveryOutcome` including
  `Suppressed { rule_id }` for the ADR-L5 audit case.
- **`(status, next_retry_at)` composite index** is the hot path for the
  retry scheduler query.
- **`STRICT` tables** per ADR-P1 §1 convention.

The DDL is added to `migrations/0001_initial.sql` (no V0 production
users yet, so amending the initial migration is fine — no second
migration file needed).

### P3.3 — Repository trait

```rust
// src/notifications/repository.rs
#[async_trait]
pub trait NotificationAttemptRepository: Send + Sync {
    /// INSERT a new row with status=Pending. Called before dispatch.
    async fn insert_pending(&self, attempt: &NotificationAttempt)
        -> Result<(), RepoError>;

    /// UPDATE an existing row from Pending to a terminal status.
    /// `next_retry_at` is `Some(t)` iff the outcome is Transient and
    /// retries remain. `Some` schedules a retry; `None` is terminal.
    async fn complete(
        &self,
        id: &NotificationAttemptId,
        receipt: DeliveryReceipt,
        next_retry_at: Option<DateTime<Utc>>,
    ) -> Result<(), RepoError>;

    /// Rows in FailedTransient with next_retry_at <= now, oldest first.
    /// The retry scheduler calls this on its tick.
    async fn list_retryable(
        &self,
        now: DateTime<Utc>,
        limit: u32,
    ) -> Result<Vec<NotificationAttempt>, RepoError>;

    /// All attempts for an incident, newest first. Operator-UI scaffolding (V0.2).
    async fn list_for_incident(
        &self,
        incident_id: &IncidentId,
    ) -> Result<Vec<NotificationAttempt>, RepoError>;
}
```

`RepoError` reuses the variant set from ADR-L4 §L4.6 (`Backend`,
`Conflict`, `NotFound`).

### P3.4 — `NotificationAttempt` struct revision

The existing in-memory struct gains retry-related fields:

```rust
pub struct NotificationAttempt {
    pub id: NotificationAttemptId,
    pub rule_id: NotificationRuleId,
    pub incident_id: IncidentId,             // NEW — was implicit in lifecycle_event
    pub lifecycle_kind: IncidentNotificationEventKind,  // discriminant only — not full event
    pub target_kind: TargetKind,             // NEW (enum: Telegram, Discord, Webhook, Stdout)
    pub target_summary: String,              // NEW — redacted

    pub status: NotificationDeliveryStatus,  // expanded to include Suppressed + FailedTransient + FailedPermanent
    pub attempt_number: u32,                 // NEW
    pub parent_attempt_id: Option<NotificationAttemptId>,  // NEW
    pub next_retry_at: Option<DateTime<Utc>>,              // NEW

    pub outcome: Option<DeliveryOutcome>,    // None while Pending
    pub external_ref: Option<ExternalMessageRef>,
    pub attempted_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
```

The old `incident_lifecycle_event: IncidentLifecycleEvent` and
`target: NotificationTarget` fields are removed:

- The full lifecycle event would duplicate `incidents` rows and carry
  redundant data; storing `incident_id` + `lifecycle_kind` is enough,
  and the consumer reconstructs the full event from the incident
  repository at retry time (per P3.7).
- The full target carries `SecretString` values that must not be
  persisted; `target_kind` + `target_summary` captures the
  not-sensitive shape.

### P3.5 — Backoff per target kind

Each notifier sender reports a transient outcome with an optional
`retry_after` field that the protocol surfaces (Telegram's API field,
Discord's `Retry-After` header, HTTP `Retry-After` for webhooks).
When present, the scheduler uses it directly.

When absent, fall back to per-kind defaults:

| Target kind | Backoff schedule (default) |
|---|---|
| Telegram | `[30s, 120s, 600s]` |
| Discord  | `[30s, 120s, 600s]` |
| Webhook  | `[30s, 120s, 600s]` |
| Stdout   | no retry (debug-only; failures are bugs) |

Telegram's `retry_after` (from `parameters.retry_after` in the API
error response) is honored when present; same for Discord's
`Retry-After` header. If the protocol-supplied delay exceeds the
default for that attempt number, use the protocol's value.

### P3.6 — Max attempts: 3 retries (4 total)

```rust
// src/runtime/config.rs
pub struct RuntimeConfig {
    pub channel_capacity: usize,
    pub shutdown_deadline_seconds: u64,
    pub notification_max_retries: u32,     // default 3
    pub notification_retry_tick_seconds: u64,  // default 10
}
```

After the 3rd retry's `Transient` outcome, the attempt is marked
`FailedPermanent` with `outcome_kind = 'Transient'` (the underlying
cause was transient, but we've exhausted retries).

### P3.7 — Retry scheduler in the consumer task

The retry scheduler is an additional `tokio::select!` branch in the
existing consumer task (ADR-S1, ADR-S3). No separate task, no new
channels. The consumer's main loop:

```rust
let mut retry_ticker =
    tokio::time::interval(Duration::from_secs(config.notification_retry_tick_seconds));
retry_ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

loop {
    tokio::select! {
        Some(batch) = rx.recv() => {
            // existing observation pipeline (ADR-S1)
        }

        _ = retry_ticker.tick() => {
            let retryable = attempts_repo
                .list_retryable(Utc::now(), 32)
                .await
                .unwrap_or_default();
            for old_attempt in retryable {
                self.process_retry(old_attempt).await;
            }
        }

        _ = shutdown.recv() => break,
    }
}
```

`process_retry`:

1. Loads the incident via `incident_repo.get(old_attempt.incident_id)`.
2. Reconstructs the lifecycle event from `old_attempt.lifecycle_kind`
   and the incident.
3. Calls `compose_notification_message(&event)` (per P3.8 — message
   regeneration).
4. Calls `notifier.dispatch_one(rule, message, target)` (the
   single-rule path, not the fan-out path).
5. Persists a new attempt row via `insert_pending` then `complete`.

The single-writer property (ADR-L4 §L4.4, ADR-S1) is preserved
naturally — the retry path runs in the same task as the primary
dispatch path.

### P3.8 — Re-render messages from incident state on retry

When a retry fires, the consumer re-renders the `NotificationMessage`
from current incident state rather than persisting the original
rendered message in the row. Reasons:

- **Cleaner schema** — no `notification_message_json` column.
- **Freshness** — if the incident has been updated (e.g. additional
  evidence accumulated) since the original attempt, the retry carries
  the latest summary.
- **Determinism** — `compose_notification_message(event)` is pure;
  same input produces the same output.

Tradeoff: small drift between attempts on time-relative phrasing
("opened 3m ago" → "opened 15m ago"). Operators benefit; nothing
breaks.

### P3.9 — Suppressed attempts are recorded

When the V0.1 notifier drops a delivery because of a `SuppressionRule`
(ADR-L5 §L5.4), it still constructs a `NotificationAttempt`:

- `status = 'Suppressed'`
- `outcome_kind = 'Suppressed'`
- `outcome_json` carries `DeliveryOutcome::Suppressed { rule_id }`
- No `next_retry_at` — suppressed deliveries are terminal, not retried.

This is the audit trail ADR-L5 §L5.4 promised. The future operator UI
displays "muted by rule X" for each suppressed attempt.

### P3.10 — `Notifier::dispatch` signature change

The current return type is `Vec<(NotificationRuleId, DeliveryReceipt)>`.
ADR-P3 changes it to:

```rust
pub async fn dispatch(
    &self,
    event: &IncidentLifecycleEvent,
    message: &NotificationMessage,
    attempts_repo: &dyn NotificationAttemptRepository,
    now: DateTime<Utc>,
) -> Vec<NotificationAttempt>;
```

`Notifier::dispatch` internally:

1. For each matching rule, build a `NotificationAttempt` with
   `status = Pending` and `attempt_number = 1`.
2. `insert_pending` for each.
3. Call the appropriate sender.
4. Build a `DeliveryReceipt`.
5. Compute `next_retry_at` if the outcome is `Transient` and retries
   remain.
6. `complete` with the receipt + `next_retry_at`.
7. Return the updated `NotificationAttempt` records.

Returning `Vec<NotificationAttempt>` (not just receipts) lets the
caller inspect retry state without re-reading the repository.

### P3.11 — Retention

`RetentionConfig` gains a fourth knob:

```rust
pub struct RetentionConfig {
    pub observations_max_age:  Option<Duration>,
    pub incidents_max_age:     Option<Duration>,
    pub suppressions_grace:    Option<Duration>,
    pub attempts_max_age:      Option<Duration>,   // NEW
    pub vacuum_interval:       Duration,
}
```

Default **30 days** — shorter than observations because attempts are
denser (every notification produces a row, plus retry rows). The sweep
in `retention::run` (ADR-P2 §P2.5) gains:

```sql
DELETE FROM notification_attempts
WHERE attempted_at < ? AND status != 'Pending';
```

The `status != 'Pending'` guard prevents the sweep from deleting an
in-flight attempt. (At V0 throughput a Pending row is gone within
seconds; this is belt-and-suspenders against an exotic stuck-Pending.)

### P3.12 — Module placement

```text
src/notifications/
├── repository.rs                       # NEW — NotificationAttemptRepository trait + RepoError
├── (existing modules unchanged)

src/storage/sqlite/
├── notification_attempt_repository.rs  # NEW — SqliteNotificationAttemptRepository

src/storage/memory/
├── notification_attempt_repository.rs  # NEW — MemoryNotificationAttemptRepository (tests)
```

Same pattern as `IncidentRepository` (`src/incidents/repository.rs`)
and `SuppressionRepository` (`src/incidents/suppression.rs`).

### Tickets

Three new tickets plus one re-scope:

- **BTH-51** — `NotificationAttemptRepository` trait + revised
  `NotificationAttempt` struct + `MemoryNotificationAttemptRepository`
  (S).
- **BTH-52** — `SqliteNotificationAttemptRepository` + amend
  `migrations/0001_initial.sql` with the table and four indexes (M).
- **BTH-53** — Retry scheduler tick in the consumer task; backoff
  defaults per target kind; max-attempts policy; suppressed-attempts
  recording; `Notifier::dispatch` signature change to accept
  `&dyn NotificationAttemptRepository` (M).

Re-scopes:

- **BTH-13** (retention task) — add `attempts_max_age` to
  `RetentionConfig` and a fourth `DELETE` in the sweep.

**Rationale.**

- **Per-row immutability** sidesteps a class of state-machine bugs:
  no row ever transitions between terminal states; retries always
  produce fresh rows.
- **Re-render on retry** keeps the schema small and messages fresh.
- **Scheduler in the consumer** preserves ADR-S1's single-writer
  property without new tasks or channels.
- **Per-target-kind backoff with API-honored `retry_after`** matches
  protocol contracts; uniform defaults are a fallback.
- **Suppressed attempts are recorded as terminal-no-retry** so ADR-L5
  §L5.4's audit promise is actually durable.

**Alternatives considered.**

- **Completed-only persistence with no retry** (the original V0
  proposal): rejected on cost/benefit — durable retry is bounded
  complexity and saves a future refactor.
- **Mutable retry on the same row** (UPDATE `attempt_number` and
  `status` in place): rejected — loses the per-row outcome history;
  more locking; row identity loses meaning across retries.
- **Persist the rendered `NotificationMessage`**: rejected — schema
  bloat; messages can drift slightly on retry but operators benefit
  from fresher info.
- **Separate scheduler task**: rejected — breaks single-writer or
  introduces an extra channel + sync layer for no real benefit at
  V0 scale.

**Spec updates queued.**

- § 11.5 — note `Notifier::dispatch` signature change; `DeliveryOutcome`
  now includes `Suppressed { rule_id }`.
- § 11.x (new) — notification attempts model.
- § 12.1 — consumer loop gains retry-ticker `select!` arm.
- § 13.1–13.3 — schema includes `notification_attempts`; pool helper
  unchanged.
- § 13.4 — `NotificationAttemptRepository` added.
- § 13.6 — retention config gains `attempts_max_age`.
- § 15.b — `src/notifications/repository.rs`,
  `src/storage/sqlite/notification_attempt_repository.rs`, and the
  memory counterpart added.
- § 21.5 / § 21.9 — inventory refreshed.





















