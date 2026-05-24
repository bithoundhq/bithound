# Bithound Domain Specification

> **Companion documents.**
> - `IMPLEMENTATION_PLAN.md` — phases, milestones, dependency graph,
>   parallelization-friendly subsets, and per-ticket estimates.
> - `TICKETS.md` — JIRA-style tickets (BTH-1 … BTH-69 and growing)
>   implementing every ADR in this spec. Each ticket lists its ADR
>   references, acceptance criteria, and blocking dependencies.
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

The full ADR text has moved into the documentation site under
[Architecture decision records](docs/src/adr/index.md). The site is the
canonical home; the index there lists every ADR, grouped by cluster.

When you add a new ADR, write the page directly under `docs/src/adr/`
and link it from `docs/src/adr/index.md` and `docs/src/SUMMARY.md`. This
section stays as a pointer.

