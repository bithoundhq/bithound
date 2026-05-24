# Bithound V0 — JIRA-style Tickets

Companion to `IMPLEMENTATION_PLAN.md` and `SPEC.md`. Every ticket
references the ADR(s) it implements; deviations from the spec require
a new ADR first.

**Ticket key:** `BTH-N`. Numbering follows phase order; gaps are not
expected to appear.

**Legend:** Type {Task, Story, Bug}; Priority {High, Medium, Low};
Estimate {S = ½–1 day, M = 1–2 days, L = 3–5 days}.

---

## Index

| Key   | Title                                                          | Phase | Estimate |
|-------|----------------------------------------------------------------|-------|----------|
| BTH-1 | Add sqlx, clap, tokio, tracing-subscriber dependencies         | 1     | S        |
| BTH-2 | Apply ADR-001 small-call cleanups                              | 1     | S        |
| BTH-3 | Add `EntitySubjectKind` discriminant for `EntityRef`           | 1     | S        |
| BTH-4 | Add `IncidentFingerprint` and draft extensions                 | 1     | M        |
| BTH-5 | Extend `ObservationPayload` with `IncidentSignal`, `Diagnosis` | 1     | S        |
| BTH-6 | `StateObservation::name()` + state `well_known` constants      | 1     | S        |
| BTH-7 | Rewrite `StateReadModel`; add `StateReadModelExt`              | 2     | M        |
| BTH-8 | Add `signals` field to `DiagnosticContext`                     | 2     | S        |
| BTH-9 | Create `migrations/0001_initial.sql` and pool helper           | 3     | M        |
| BTH-10| `ObservationStore` trait + `StoreError`                        | 3     | S        |
| BTH-11| `SqliteObservationStore` impl                                  | 3     | M        |
| BTH-12| `IncidentRepository` trait + `SqliteIncidentRepository`        | 3     | M        |
| BTH-13| Retention background task                                      | 3     | S        |
| BTH-14| In-memory test impls for stores                                | 3     | S        |
| BTH-15| `IncidentKindSpec`, `KindRegistry`, validation errors          | 4     | M        |
| BTH-16| `default_kinds.toml` + `well_known` incident-kind constants    | 4     | S        |
| BTH-17| `IncidentCommand`, `HandleOutcome`, `EngineError`              | 5     | S        |
| BTH-18| `IncidentEngine` struct, `new()`, state management             | 5     | M        |
| BTH-19| `IncidentEngine::handle()` decision tree                       | 5     | L        |
| BTH-20| `Projection` trait + `ProjectionError`                         | 6     | S        |
| BTH-21| `StateProjection`                                              | 6     | M        |
| BTH-22| `MetricProjection` with bounded ring                           | 6     | M        |
| BTH-23| `HealthProjection` + `CapabilityProjection`                    | 6     | M        |
| BTH-24| `HeartbeatProjection` + `IncidentSignalProjection`             | 6     | M        |
| BTH-25| `ReadModelStore` assembler + six trait impls                   | 6     | M        |
| BTH-26| Collector traits + `BatchSink` + sidecar_id on context         | 7     | M        |
| BTH-27| `BitcoinRpcClient` + `RpcError`                                | 7     | M        |
| BTH-28| `BitcoinCoreRpcCollector`                                      | 7     | L        |
| BTH-29| Implement `WebhookSender`                                      | 8     | M        |
| BTH-30| Implement `TelegramSender`                                     | 8     | M        |
| BTH-31| Implement `DiscordSender`                                      | 8     | M        |
| BTH-32| Config types + `clap` CLI surface                              | 9     | M        |
| BTH-33| TOML loading, env overrides, secrets, validation               | 9     | L        |
| BTH-34| Collector supervisor module                                    | 10    | M        |
| BTH-35| Pipeline consumer module                                       | 10    | L        |
| BTH-36| Bootstrap module — build collectors from config                | 10    | M        |
| BTH-37| `runtime::run()` + `main.rs` bootstrap                         | 10    | M        |
| BTH-38| `BitcoinTipLagRule` (catalog A1)                               | 11    | M        |
| BTH-39| `BitcoinPeerStarvationRule` (catalog A3)                       | 11    | M        |
| BTH-40| End-to-end integration test (regtest)                          | 12    | L        |
| BTH-41| README + operator docs update                                  | 12    | M        |
| BTH-42| Smart-constructor scaffolding (`parse_dotted_name`, `ParseDottedNameError`) | D | S |
| BTH-43| Migrate `IncidentKind` to smart constructor                    | D     | S        |
| BTH-44| Migrate `MetricName` and `SignalName`                          | D     | S        |
| BTH-45| Migrate `StateName` and `CapabilityName`                       | D     | S        |
| BTH-46| Migrate remaining name newtypes; remove compat helpers         | D     | M        |
| BTH-47| Unvalidated/Validated draft split + `KindRegistry::validate`   | D     | M        |
| BTH-48| Promote `ActorId`; extend `IncidentCommand` (Ack/Resolve stubs)| D     | S        |
| BTH-49| `SuppressionCommand` + `SuppressionService` trait              | D     | S        |
| BTH-50| Per-context `events.rs` modules + top-level `DomainEvent`      | D     | M        |
| BTH-51| `NotificationAttemptRepository` trait + revised `NotificationAttempt` + memory impl | 3 | S |
| BTH-52| `SqliteNotificationAttemptRepository` + amend `0001_initial.sql` with `notification_attempts` table | 3 | M |
| BTH-53| (V0.1) Retry scheduler + backoff defaults in the notification worker | V0.1 | M |
| BTH-54| Identity refinements — `EntityRef::Sidecar` + sub-entity scoping (ADR-N1) | 1 | S |
| BTH-55| Notification worker task (ADR-N2) — dispatch out of consumer | 10 | M |
| BTH-56| axum HTTP server bootstrap + graceful shutdown (ADR-A1) | A | S |
| BTH-57| V0 operator API endpoints (`/health`, `/incidents/open`, `/incidents/:id`, `/incidents/:id/evidence`) | A | M |
| BTH-58| `BitcoinRpcUnreachableRule` + `bitcoin.rpc_unreachable` kind | 11 | S |
| BTH-59| Vendor LND `.proto` files + tonic/prost/tonic-build deps + `build.rs` | V0.8 | L |
| BTH-60| `StateObservation::LndChannel(LndChannelState)` + `lnd.channel_detail` constant + parity | V0.8 | S |
| BTH-61| `lnd.channel_inactive` + `lnd.chain_backend_lag` kinds + bidirectional parity test | V0.8 | S |
| BTH-62| `LndGrpcClient` (tonic + macaroon header + LND-cert-only TLS + error mapping) | V0.8 | M |
| BTH-63| `LndGrpcPollingCollector` (`impl PollingCollector`, `peer_online` cross-reference) | V0.8 | M |
| BTH-64| `LndChannelInactiveRule` (catalog B1) | V0.8 | M |
| BTH-65| `LndChainBackendLagRule` (catalog B6) | V0.8 | M |
| BTH-66| `bithound.lnd_*` internal incident kinds (operability) | V0.8 | S |
| BTH-67| End-to-end test for B1 + B6 via Polar regtest | V0.8 | L |
| BTH-68| v0.0.8.0 docs refresh + CHANGELOG + VERSION bump + catalog status flips | V0.8 | S |

**Re-scopes from D-cluster (ADR-D4 supersedes ADR-L4 §L4.2):**
- BTH-17 — was "IncidentCommand + HandleOutcome + EngineError"; now defines `IncidentEvent` (no `HandleOutcome`).
- BTH-19 — was "engine handle decision tree returning HandleOutcome"; now returns `Vec<IncidentEvent>`.
- BTH-35 — was "consumer reads outcome fields + dispatches"; now consumer pattern-matches events AND ends at "enqueue notification" (dispatch moves to BTH-55 per ADR-N2).

**Re-scopes from ADR-P3 (notification attempts persistence):**
- BTH-13 — retention task gains `attempts_max_age` knob and a fourth `DELETE` in the sweep.

**Re-scopes from architecture review (ADR-N1, ADR-N2, ADR-A1, ADR-P3 audit-only revision):**
- BTH-38 — rule renamed to `BitcoinTipLagOrIbdStalledRule` implementing `bitcoin.tip_lag_or_ibd_stalled` (combines catalog A1 + A2).
- BTH-39 — rule renamed to `BitcoinNoPeersRule` implementing `bitcoin.no_peers` (zero outbound peers; tighter than catalog A3 which was <8).
- BTH-29/30/31 — V0 needs at most one sender. **Webhook (BTH-29)** is the V0 target; Telegram (BTH-30) and Discord (BTH-31) move to V0.1.
- BTH-53 — **moved to V0.1.** Retry scheduler lands in the notification worker, not the consumer (per ADR-N2 §N2.5).
- New tickets BTH-54..BTH-58 added per the architecture review.

**GitHub issue numbering note (BTH-42 onwards):** the BTH-N → GitHub
issue-#N mapping holds for BTH-1 through BTH-41. From BTH-42 onwards
the mapping shifts because PRs #42–#48 already consumed those numbers
when the D-cluster issues were created:

| Ticket | GitHub issue |
|--------|--------------|
| BTH-42 | #49          |
| BTH-43 | #50          |
| BTH-44 | #51          |
| BTH-45 | #52          |
| BTH-46 | #53          |
| BTH-47 | #54          |
| BTH-48 | #55          |
| BTH-49 | #56          |
| BTH-50 | #57          |
| BTH-51 | #63          |
| BTH-52 | #64          |
| BTH-53 | #65          |
| BTH-54 | #67          |
| BTH-55 | #68          |
| BTH-56 | #69          |
| BTH-57 | #70          |
| BTH-58 | #71          |
| BTH-59 | #112         |
| BTH-60 | #113         |
| BTH-61 | #114         |
| BTH-62 | #115         |
| BTH-63 | #116         |
| BTH-64 | #117         |
| BTH-65 | #118         |
| BTH-66 | #119         |
| BTH-67 | #120         |
| BTH-68 | #121         |

(PRs #58–#62 consumed the matching numbers when ADR-D4 docs / BTH-7 /
BTH-8 / BTH-4-recover / CLAUDE.md-gotcha landed. PR #66 consumed when
ADR-P3 docs landed. PRs #72–#109 consumed the matching numbers
through Phase A. PR #110 (ADR-E1/C4-defer) and PR #111 (these V0.8
tickets) consumed those numbers; BTH-59..68 land at issues
#112–#121.)

Issue bodies use the GitHub numbers (e.g. "Blocked by: #49 (BTH-42)")
so cross-references resolve via GitHub autolinking.

---

# Phase 1 — Foundation cleanups

## BTH-1: Add sqlx, clap, tokio, tracing-subscriber dependencies

**Type** Task • **Priority** High • **Estimate** S • **Component** build
**ADRs** P1, X1, S3 §S3.1 • **Blocked by** — • **Blocks** BTH-9, BTH-11, BTH-32, BTH-37

### Description
Update `Cargo.toml` to add the dependencies required by subsequent
tickets. Also adds a minimal CI workflow so quality gates fire on
every PR going forward.

New deps:
- `sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "tls-rustls", "sqlite", "chrono", "uuid", "migrate"] }`
- `clap = { version = "4", features = ["derive"] }`
- `tokio = { version = "1", features = ["full"] }` (currently transitive only)
- `tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }`
- `toml = "0.8"`
- `thiserror = "1"`

Add `.github/workflows/ci.yml` running `cargo fmt --check`, `cargo
clippy -- -D warnings`, `cargo build`, `cargo test`.

### Acceptance criteria
- [ ] `Cargo.toml` lists the dependencies above with exact features
- [ ] `cargo check` and `cargo build` succeed with no behavioral changes
- [ ] `.github/workflows/ci.yml` exists and runs fmt/clippy/build/test
- [ ] No source files modified beyond what the build requires

---

## BTH-2: Apply ADR-001 small-call cleanups

**Type** Task • **Priority** High • **Estimate** S • **Component** types
**ADRs** 001 • **Blocked by** — • **Blocks** BTH-4, BTH-8, BTH-15

### Description
Bundle the six small calls from ADR-001:

1. Rename `IncidentStatus::Supressed` → `Suppressed` everywhere
   (`src/incidents/types.rs:44` and any callers).
2. Change `Incident.signal_observation_ids` from `Option<ObservationId>`
   to `Vec<ObservationId>`.
3. Mark diagnostics submodules public: `src/diagnostics/mod.rs`
   becomes `pub mod traits; pub mod types;` and the contained items
   become reachable from `main.rs` and siblings.
4. Reserve a future-compatible empty entry for `signals: &dyn
   IncidentSignalReadModel` in `DiagnosticContext` (BTH-8 adds the
   real wiring once `IncidentSignalReadModel` is in scope from
   the trait inventory).
5. Add `src/incidents/well_known.rs` (empty stub — populated in
   BTH-16); add `mod well_known;` to `src/incidents/mod.rs`.
6. Add module declarations `pub mod engine;` and `pub mod
   repository;` to `src/incidents/mod.rs` pointing at empty stub files
   (engine and repo land in later tickets).

### Acceptance criteria
- [ ] No occurrence of `Supressed` remains
- [ ] `Incident.signal_observation_ids: Vec<ObservationId>` compiles
- [ ] `cargo doc --no-deps` lists `bithound::diagnostics::{traits, types}`
- [ ] `bithound::incidents::well_known` exists (may be empty)
- [ ] `bithound::incidents::{engine, repository}` modules exist (may be empty)
- [ ] All existing tests pass

---

## BTH-3: Add `EntitySubjectKind` discriminant for `EntityRef`

**Type** Task • **Priority** High • **Estimate** S • **Component** shared
**ADRs** L1 §3 • **Blocked by** BTH-2 • **Blocks** BTH-4, BTH-15

### Description
Add a named discriminant enum for `EntityRef` so the kind registry can
declare which subject kinds an `IncidentKind` permits without using
`std::mem::discriminant`.

```rust
// src/shared/types.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntitySubjectKind {
    Host, BitcoinNode, BitcoinPeer,
    LndNode, LndPeer, LndChannel, LndInvoice,
}

impl EntityRef {
    pub fn subject_kind(&self) -> EntitySubjectKind { /* match */ }
}
```

### Acceptance criteria
- [ ] `EntitySubjectKind` enum present with all seven variants
- [ ] `EntityRef::subject_kind()` exhaustively matches; compiler error
      if a future `EntityRef` variant is added without updating this
- [ ] `serde` round-trip test for the enum
- [ ] Unit test asserts `EntitySubjectKind::BitcoinNode ==
      EntityRef::BitcoinNode(_).subject_kind()`

---

## BTH-4: Add `IncidentFingerprint` and `IncidentSignalDraft` extensions

**Type** Task • **Priority** High • **Estimate** M • **Component** incidents
**ADRs** L1 §§1–2 • **Blocked by** BTH-2, BTH-3 • **Blocks** BTH-15, BTH-17

### Description
Introduce the structured fingerprint used by the engine for incident
identity, and extend `IncidentSignalDraft` to carry the inputs.

```rust
// src/incidents/types.rs
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IncidentFingerprint {
    pub subject: EntityRef,
    pub kind: IncidentKind,
    pub dimension: Option<String>,
}

impl IncidentFingerprint {
    pub fn as_key(&self) -> String { /* stable string form for indexing */ }
}

// Also add `fingerprint: IncidentFingerprint` to Incident struct.
```

```rust
// src/diagnostics/types.rs
pub struct IncidentSignalDraft {
    pub subject: EntityRef,
    pub signal: SignalName,
    pub kind: IncidentKind,            // NEW
    pub dimension: Option<String>,     // NEW
    pub severity: SignalSeverity,
    pub status: SignalStatus,
    pub confidence: Confidence,
    pub evidence: Vec<EvidenceRef>,
}
```

Update `Incident::new()` (or constructor pattern) to take a
fingerprint. Add a helper `fn compute_fingerprint(draft: &IncidentSignalDraft) -> IncidentFingerprint`.

### Acceptance criteria
- [ ] `IncidentFingerprint` defined with `as_key()` returning
      `"<subject_kind>|<subject_id>|<incident_kind>|<dimension or '-'>"`
- [ ] `Incident.fingerprint` field added
- [ ] `IncidentSignalDraft.kind` and `.dimension` added
- [ ] Unit tests cover fingerprint equality and `as_key` stability
- [ ] All existing tests still pass

---

## BTH-5: Extend `ObservationPayload` with `IncidentSignal`, `Diagnosis`

**Type** Task • **Priority** High • **Estimate** S • **Component** observations
**ADRs** R2 • **Blocked by** BTH-2 • **Blocks** BTH-19, BTH-24

### Description
Promote `IncidentSignalObservation` and `DiagnosisObservation` from
free-standing types to `ObservationPayload` variants. Add the matching
constructor helpers on `Observation`.

```rust
pub enum ObservationPayload {
    Capability(CapabilityObservation),
    Diagnosis(DiagnosisObservation),               // NEW
    Event(EventObservation),
    Heartbeat(HeartbeatObservation),
    Health(HealthCheckObservation),
    IncidentSignal(IncidentSignalObservation),     // NEW
    Inventory(InventoryObservation),
    Metric(MetricObservation),
    State(StateObservation),
    Transition(TransitionObservation),
}

impl Observation {
    pub fn incident_signal(ctx: ObservationContext,
                           signal: IncidentSignalObservation,
                           attributes: Attributes) -> Self { … }
    pub fn diagnosis(ctx: ObservationContext,
                     diagnosis: DiagnosisObservation,
                     attributes: Attributes) -> Self { … }
}
```

### Acceptance criteria
- [ ] `ObservationPayload` has ten variants
- [ ] `Observation::incident_signal` and `::diagnosis` constructors exist
- [ ] Round-trip serde test for each new variant
- [ ] No other code regresses (existing observation types untouched)

---

## BTH-6: `StateObservation::name()` + state `well_known` constants

**Type** Task • **Priority** High • **Estimate** S • **Component** observations
**ADRs** R1 §R1.2 • **Blocked by** BTH-2 • **Blocks** BTH-7, BTH-21

### Description
Each `StateObservation` variant gets a canonical name. Add a parity
unit test between the variant arms and the constants.

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

### Acceptance criteria
- [ ] `StateObservation::name()` method present
- [ ] `well_known` constants present for all eight current variants
- [ ] Parity unit test fails if a variant is added without a `name`
      arm or vice versa

---

# Phase 2 — Read-model trait surface

## BTH-7: Rewrite `StateReadModel`; add `StateReadModelExt`

**Type** Task • **Priority** High • **Estimate** M • **Component** read_models
**ADRs** R1 §§R1.1, R1.3 • **Blocked by** BTH-6 • **Blocks** BTH-21, BTH-25

### Description
Replace the per-variant methods on `StateReadModel` with generic ones,
and add an extension trait with typed helpers.

```rust
// src/read_models/traits/state.rs (REPLACE existing contents)
pub trait StateReadModel: Send + Sync + std::fmt::Debug {
    fn latest_state(&self, subject: &EntityRef, name: &StateName)
        -> Option<Projected<StateObservation>>;
    fn states_for(&self, subject: &EntityRef)
        -> Vec<Projected<StateObservation>>;
}
```

```rust
// src/read_models/traits/state_ext.rs (NEW)
pub trait StateReadModelExt: StateReadModel {
    fn bitcoin_blockchain(&self, node: &BitcoinNodeId)
        -> Option<Projected<BitcoinBlockchainState>> { /* unwrap variant */ }
    // … one helper per StateObservation variant
}
impl<T: StateReadModel + ?Sized> StateReadModelExt for T {}
```

### Acceptance criteria
- [ ] `StateReadModel` has exactly two methods (`latest_state`, `states_for`)
- [ ] `StateReadModelExt` provides typed helpers for all eight variants
- [ ] All existing callers of the old per-variant methods updated
- [ ] Test that a value retrieved via `latest_state` matches a value
      retrieved via the corresponding `StateReadModelExt` helper

---

## BTH-8: Add `signals` field to `DiagnosticContext`

**Type** Task • **Priority** High • **Estimate** S • **Component** diagnostics
**ADRs** 001 §4 • **Blocked by** BTH-2 • **Blocks** BTH-19, BTH-38

### Description
Add `signals: &'a dyn IncidentSignalReadModel` to `DiagnosticContext`
so rules emitting `Cleared` signals can see what's currently active.

### Acceptance criteria
- [ ] `DiagnosticContext` has six trait-object fields (state, metrics,
      health, capabilities, heartbeats, signals)
- [ ] `cargo doc --no-deps` shows the new field
- [ ] No regressions

---

# Phase 3 — Storage layer (SQLite)

## BTH-9: `migrations/0001_initial.sql` + sqlx pool helper

**Type** Task • **Priority** High • **Estimate** M • **Component** storage
**ADRs** P1 • **Blocked by** BTH-1 • **Blocks** BTH-11, BTH-12

### Description
Create the initial schema migration and an `open_pool` helper that
sets up WAL mode, NORMAL synchronous, and runs migrations.

Files:
- `migrations/0001_initial.sql` — full DDL from ADR-P1.
- `src/storage/mod.rs` — module declarations.
- `src/storage/sqlite/mod.rs` — `pub async fn open_pool(path: &Path) -> Result<SqlitePool, StoreError>`.

### Acceptance criteria
- [ ] `migrations/0001_initial.sql` creates three STRICT tables with
      ADR-P1's columns and indexes
- [ ] `open_pool` applies PRAGMAs and runs migrations
- [ ] Integration test: open a fresh temp-file DB, run `open_pool`,
      assert tables exist via `sqlite_master`
- [ ] Re-opening an existing DB is idempotent (migrations skip)

---

## BTH-10: `ObservationStore` trait + `StoreError`

**Type** Task • **Priority** High • **Estimate** S • **Component** storage
**ADRs** P2 §§P2.1, P2.2 • **Blocked by** BTH-1 • **Blocks** BTH-11, BTH-35

### Description
Define the trait and error type in `src/storage/traits.rs`.

```rust
#[async_trait]
pub trait ObservationStore: Send + Sync {
    async fn append_many(&self, batch: &[Observation]) -> Result<(), StoreError>;
    async fn iter_since(&self, since: DateTime<Utc>)
        -> Result<BoxStream<'_, Result<Observation, StoreError>>, StoreError>;

    async fn append(&self, obs: &Observation) -> Result<(), StoreError> {
        self.append_many(std::slice::from_ref(obs)).await
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("io: {0}")] Io(#[from] std::io::Error),
    #[error("database: {0}")] Database(#[from] sqlx::Error),
    #[error("serialization: {0}")] Serialization(#[from] serde_json::Error),
    #[error("corruption: {0}")] Corruption(String),
    #[error("not initialized")] NotInitialized,
}
```

### Acceptance criteria
- [ ] Trait, default `append` shim, and error type present
- [ ] `thiserror` annotations compile
- [ ] Doc-comments document the trait contract

---

## BTH-11: `SqliteObservationStore` impl

**Type** Task • **Priority** High • **Estimate** M • **Component** storage
**ADRs** P1, P2 §P2.3 • **Blocked by** BTH-9, BTH-10 • **Blocks** BTH-35

### Description
Implement the trait against sqlx + SQLite.

- `append_many`: single transaction, INSERT each row with bound params.
- `iter_since`: SELECT with `WHERE observed_at >= ? ORDER BY observed_at`,
  return a `BoxStream` deserializing rows lazily.

### Acceptance criteria
- [ ] `SqliteObservationStore::open(path)` constructs from a pool path
- [ ] `append_many` round-trips: write batch, then `iter_since(epoch)`
      yields the same observations in order
- [ ] `iter_since` respects the timestamp filter
- [ ] All ten payload variants survive round-trip (parametrized test)
- [ ] Concurrent `append_many` calls serialize correctly under WAL

---

## BTH-12: `IncidentRepository` trait + `SqliteIncidentRepository`

**Type** Task • **Priority** High • **Estimate** M • **Component** storage
**ADRs** L4 §L4.6, P1, P2 §P2.2 • **Blocked by** BTH-9, BTH-4 • **Blocks** BTH-18, BTH-37

### Description
Add the trait to `src/incidents/repository.rs` and the SQLite impl to
`src/storage/sqlite/incident_repository.rs`.

```rust
#[async_trait]
pub trait IncidentRepository: Send + Sync {
    async fn load_open(&self) -> Result<Vec<Incident>, RepoError>;
    async fn save(&self, incident: &Incident) -> Result<(), RepoError>;
}

#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    #[error("backend: {0}")] Backend(String),
    #[error("conflict on incident {id:?}")] Conflict { id: IncidentId },
    #[error("incident not found: {id:?}")] NotFound { id: IncidentId },
}
```

Impl: UPSERT on `save` (`INSERT … ON CONFLICT(id) DO UPDATE SET …`);
`load_open` filters by `status != 'Resolved'`.

### Acceptance criteria
- [ ] Trait + error type live in `src/incidents/repository.rs`
- [ ] `SqliteIncidentRepository::open(pool)` returns a working impl
- [ ] `save` on an existing ID replaces the row atomically
- [ ] `load_open` returns only open + acknowledged incidents
- [ ] Round-trip test covers all four `IncidentStatus` values
      (Suppressed is included for forward compat though unused in V0)

---

## BTH-13: Retention background task

**Type** Task • **Priority** Medium • **Estimate** S • **Component** storage
**ADRs** P2 §P2.5 • **Blocked by** BTH-9 • **Blocks** BTH-37

### Description
Add `src/storage/retention.rs` with `run(pool, config, shutdown_rx)`
that sweeps old rows and runs `VACUUM` on the configured interval.

```rust
pub struct RetentionConfig {
    pub observations_max_age: Option<Duration>,
    pub incidents_max_age:    Option<Duration>,
    pub suppressions_grace:   Option<Duration>,
    pub vacuum_interval:      Duration,
}

pub async fn run(pool: SqlitePool, config: RetentionConfig,
                 mut shutdown: broadcast::Receiver<()>);
```

### Acceptance criteria
- [ ] `RetentionConfig` and `run` defined per ADR-P2 §P2.5
- [ ] `None` ages disable retention for that table
- [ ] Test that observations older than `observations_max_age` are
      deleted after one sweep
- [ ] Test that resolved incidents older than `incidents_max_age` are deleted
- [ ] Shutdown signal exits the loop within 1s

---

## BTH-14: In-memory test impls for stores

**Type** Task • **Priority** Medium • **Estimate** S • **Component** storage
**ADRs** P2 §P2.7 • **Blocked by** BTH-10, BTH-12 • **Blocks** BTH-35 (test path)

### Description
Implement `MemoryObservationStore` and `MemoryIncidentRepository`
backed by `tokio::sync::Mutex<Vec<…>>` for use in integration tests.

### Acceptance criteria
- [ ] Both types implement their respective traits
- [ ] Trait conformance tests reuse the same suite as the SQLite impls
- [ ] Live in `src/storage/memory/`

---

# Phase 4 — Kind registry

## BTH-15: `IncidentKindSpec`, `KindRegistry`, validation errors

**Type** Task • **Priority** High • **Estimate** M • **Component** incidents
**ADRs** L1 §§3–4, L2 §L2.2 • **Blocked by** BTH-3, BTH-4 • **Blocks** BTH-18, BTH-19

### Description
Add `src/incidents/kinds.rs` with the registry types and loader.

```rust
pub struct IncidentKindSpec {
    pub name: String,
    pub allowed_subjects: Vec<EntitySubjectKind>,
    pub allows_dimension: bool,
    pub dimension_label: Option<String>,
    pub min_open_confidence: Confidence,        // default Medium
    pub source: KindSource,
}

pub enum KindSource { Builtin, UserConfig }

pub struct KindRegistry { kinds: HashMap<IncidentKind, IncidentKindSpec> }

impl KindRegistry {
    pub fn load(user_config: Option<&Path>) -> Result<Self, RegistryError>;
    pub fn lookup(&self, kind: &IncidentKind) -> Option<&IncidentKindSpec>;
    pub fn validate_draft(&self, draft: &IncidentSignalDraft) -> Result<(), DraftError>;
}

pub enum RegistryError { /* per ADR-L1 §4 */ }
pub enum DraftError    { /* per ADR-L1 §4 */ }
```

### Acceptance criteria
- [ ] All four error variants on each enum present
- [ ] `validate_draft` enforces: known kind, allowed subject, dimension
      required/forbidden matches spec
- [ ] Built-in vs user-config layering: user-config attempting to
      override a built-in returns `CannotOverrideBuiltin`
- [ ] Unit tests for each `DraftError` variant
- [ ] Unit test for `validate_draft` happy path

---

## BTH-16: `default_kinds.toml` + `well_known` incident-kind constants

**Type** Task • **Priority** High • **Estimate** S • **Component** incidents
**ADRs** L1 §§5–6 • **Blocked by** BTH-15 • **Blocks** BTH-38, BTH-39

### Description
Create `config/default_kinds.toml` with V0 kinds (covering catalog
A1, A2, A3, A4, X1 and reserving slots for LND), and populate
`src/incidents/well_known.rs` with matching `&'static str` constants.
A unit test asserts the TOML and the constants stay in sync.

V0 kinds to include:
- `bitcoin.tip_lag` — BitcoinNode subject, no dimension
- `bitcoin.ibd_stall` — BitcoinNode, no dimension
- `bitcoin.peer_starvation` — BitcoinNode, no dimension
- `bitcoin.mempool_full` — BitcoinNode, no dimension
- `bitcoin.reorg_deep` — BitcoinNode, no dimension
- `host.disk_exhaustion` — Host subject, dimension = "mount_path"
- `lnd.channel_inactive` — LndChannel subject, no dimension (V0.1)
- `lnd.htlc_stuck` — LndChannel subject, dimension = "payment_hash" (V0.1)
- `sidecar.collector_failing` — Host subject, dimension = "collector_id"

### Acceptance criteria
- [ ] `config/default_kinds.toml` parses cleanly through `KindRegistry::load`
- [ ] `src/incidents/well_known.rs` const names match TOML names exactly
- [ ] Parity test fails if either is updated without the other

---

# Phase 5 — Incident engine

## BTH-17: `IncidentCommand`, `IncidentEvent`, `EngineError`

**Type** Task • **Priority** High • **Estimate** S • **Component** incidents
**ADRs** L4 §L4.1, **D3**, **D4** (supersedes L4 §L4.2) • **Blocked by** BTH-4, BTH-47 (D1), BTH-48 (D3) • **Blocks** BTH-18, BTH-19

### Description

**Re-scoped by ADR-D4.** Previously defined `HandleOutcome`; that shape
has been superseded by `Vec<IncidentEvent>` for cloud-readiness.

Add the command, event, and error types in `src/incidents/`:

```rust
// src/incidents/engine.rs (ADR-D3)
pub enum IncidentCommand {
    RecordSignal(UnvalidatedIncidentSignalDraft),       // ADR-D1
    Acknowledge { id: IncidentId, by: ActorId, at: DateTime<Utc> },
    Resolve     { id: IncidentId, by: ActorId, at: DateTime<Utc>, reason: String },
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("draft validation: {0:?}")] Draft(DraftError),
    #[error("command not yet implemented: {0}")] NotYetImplemented(&'static str),
}

// src/incidents/events.rs (ADR-D4)
pub enum IncidentEvent {
    SignalRecorded(Observation),
    IncidentTouched(Incident),
    Lifecycle(IncidentLifecycleEvent),
    DraftRejected { rule_id: String, error: DraftError },
    DraftBelowConfidenceFloor {
        kind: IncidentKind, confidence: Confidence, floor: Confidence,
    },
}
```

### Acceptance criteria
- [ ] `IncidentCommand` includes RecordSignal (with `UnvalidatedIncidentSignalDraft`), Acknowledge, Resolve
- [ ] `IncidentEvent` includes all five variants from ADR-D4
- [ ] `EngineError` includes `Draft` and `NotYetImplemented(&'static str)`
- [ ] All types `Debug + Clone` where appropriate; `IncidentEvent` is `Serialize` (for cloud sync) but not `Deserialize`
- [ ] `HandleOutcome` is **not** added (it was the superseded shape)

---

## BTH-18: `IncidentEngine` struct + `new()` + state management

**Type** Task • **Priority** High • **Estimate** M • **Component** incidents
**ADRs** L4 §§L4.4–L4.6 • **Blocked by** BTH-15, BTH-17, BTH-12 • **Blocks** BTH-19

### Description
Implement the engine struct and constructor. State is an in-memory
`HashMap<IncidentFingerprint, Incident>` rebuilt from
`IncidentRepository::load_open` at startup.

```rust
pub struct IncidentEngine {
    kinds: KindRegistry,
    sidecar_id: SidecarId,
    open_incidents: HashMap<IncidentFingerprint, Incident>,
}

impl IncidentEngine {
    pub fn new(kinds: KindRegistry, sidecar_id: SidecarId,
               open_incidents: Vec<Incident>) -> Self {
        let map = open_incidents.into_iter()
            .map(|inc| (inc.fingerprint.clone(), inc))
            .collect();
        Self { kinds, sidecar_id, open_incidents: map }
    }
}
```

### Acceptance criteria
- [ ] `IncidentEngine::new` builds the map from a Vec
- [ ] Test: passing N open incidents results in a map of size N keyed
      by fingerprint
- [ ] Test: duplicate fingerprints in input panic or are deduplicated
      (decide; ADR-L4 §L4.4 implies they shouldn't exist)

---

## BTH-19: `IncidentEngine::handle()` decision tree

**Type** Story • **Priority** High • **Estimate** L • **Component** incidents
**ADRs** L1, L2, L3, L4 §L4.1, **D1**, **D3**, **D4** • **Blocked by** BTH-5, BTH-8, BTH-18, BTH-47 (D1), BTH-48 (D3) • **Blocks** BTH-37

### Description

**Re-scoped by ADR-D4.** Engine now returns `Vec<IncidentEvent>` instead
of `HandleOutcome`. The decision tree is unchanged in spirit but events
are emitted per-state-change rather than packed into a struct.

Implement the full decision tree per § 10.5.1 of SPEC.md (event flow):

```rust
impl IncidentEngine {
    pub fn handle(&mut self, cmd: IncidentCommand, now: DateTime<Utc>)
        -> Result<Vec<IncidentEvent>, EngineError>;
}
```

Per ADR-D4, the event-ordering invariant within a single `handle()`
call is `SignalRecorded` → `IncidentTouched` → `Lifecycle`.

### Acceptance criteria
- [ ] Validation (via `KindRegistry::validate` per ADR-D1) rejects
      malformed unvalidated drafts with `EngineError::Draft`. No
      events are emitted on validation failure.
- [ ] Active draft with no open incident, confidence ≥ floor → emits
      [`SignalRecorded(obs)`, `IncidentTouched(new)`, `Lifecycle(Opened)`]
      in that order.
- [ ] Active draft with confidence below `min_open_confidence` → emits
      [`SignalRecorded(obs)`, `DraftBelowConfidenceFloor{…}`]. No
      `IncidentTouched` or `Lifecycle`.
- [ ] Active draft on existing open incident:
  - Severity unchanged → emits [`SignalRecorded`, `IncidentTouched`]
    (no `Lifecycle`). `updated_at` bumped.
  - Severity strictly increased → emits [`SignalRecorded`,
    `IncidentTouched`, `Lifecycle(Escalated{prev, new})`].
- [ ] Active draft on resolved fingerprint (ADR-L2 §L2.3) → new
      incident; emits the open-flow events for a brand-new incident.
- [ ] Cleared draft on open incident → emits [`SignalRecorded`,
      `IncidentTouched(resolved)`, `Lifecycle(Resolved)`].
- [ ] Cleared draft with no open incident → emits [`SignalRecorded`]
      only (persist-plus-no-op).
- [ ] `Acknowledge` and `Resolve` commands return
      `Err(EngineError::NotYetImplemented(name))` (ADR-D3 stubs).
- [ ] Test matrix covers all branches (≥ 12 unit tests). Each test
      asserts on the event sequence, not on struct fields.

---

# Phase 6 — Read-model store

## BTH-20: `Projection` trait + `ProjectionError`

**Type** Task • **Priority** High • **Estimate** S • **Component** read_models
**ADRs** R1 §R1.4 • **Blocked by** — • **Blocks** BTH-21–24

### Description
Add `src/read_models/projections/mod.rs` with the trait and error.

```rust
pub trait Projection: Send + Sync + std::fmt::Debug {
    fn apply(&mut self, obs: &Observation) -> Result<(), ProjectionError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectionError {
    #[error("invalid payload: {0}")] InvalidPayload(String),
    #[error("internal consistency: {0}")] InternalConsistency(String),
}
```

### Acceptance criteria
- [ ] Trait + error compile, doc-commented

---

## BTH-21: `StateProjection`

**Type** Task • **Priority** High • **Estimate** M • **Component** read_models
**ADRs** R1 §R1.4 • **Blocked by** BTH-6, BTH-7, BTH-20 • **Blocks** BTH-25

### Description
Add `src/read_models/projections/state.rs`. Stores
`HashMap<(EntityRef, StateName), Projected<StateObservation>>` and
applies state observations.

Apply path: only `ObservationPayload::State(_)` is consumed; key is
`(obs.subject, state.name())`. Existing entry is overwritten when a
newer `observed_at` arrives; older observations are dropped.

Query helpers `get_latest` and `for_subject` expose what the
`StateReadModel` trait needs.

### Acceptance criteria
- [ ] `StateProjection::default()` produces an empty store
- [ ] `apply` is idempotent for an identical observation
- [ ] Latest-write-wins by `observed_at` (out-of-order observations
      don't overwrite newer state)
- [ ] Each of the eight state variants round-trips through apply →
      get_latest

---

## BTH-22: `MetricProjection` with bounded ring

**Type** Task • **Priority** High • **Estimate** M • **Component** read_models
**ADRs** R1, R3 §R3.4 • **Blocked by** BTH-20 • **Blocks** BTH-25

### Description
Stores `HashMap<(EntityRef, MetricName), VecDeque<Projected<MetricObservation>>>`
with configurable per-series capacity (default 1000). On apply, push
to the back; if `len() > capacity`, pop the front.

Query helpers cover `latest_metric`, `metric_samples_since`, and
`unchanged_for` (returns the run of latest equal-valued samples).

### Acceptance criteria
- [ ] Capacity from config; default 1000
- [ ] Eviction is FIFO when capacity is exceeded
- [ ] `metric_samples_since` honours timestamp filter and ordering
- [ ] `unchanged_for` returns at least the latest sample if no change has occurred
- [ ] Property test: capacity invariant holds after 10× capacity inserts

---

## BTH-23: `HealthProjection` + `CapabilityProjection`

**Type** Task • **Priority** High • **Estimate** M • **Component** read_models
**ADRs** R1 • **Blocked by** BTH-20 • **Blocks** BTH-25

### Description
Two projections, latest-only per `(subject, key)`.

- `HealthProjection`: key = `(EntityRef, HealthTargetId)`.
- `CapabilityProjection`: key = `(EntityRef, CapabilityName)`.

### Acceptance criteria
- [ ] Both projections implement `Projection`
- [ ] Latest-write-wins by `observed_at`
- [ ] Query methods return `Option<Projected<…>>` and a per-subject scan

---

## BTH-24: `HeartbeatProjection` + `IncidentSignalProjection`

**Type** Task • **Priority** High • **Estimate** M • **Component** read_models
**ADRs** R1, R3 §R3.4 • **Blocked by** BTH-5, BTH-20 • **Blocks** BTH-25

### Description
Two projections.

`HeartbeatProjection`: sidecar-scoped, no key. Latest heartbeat + a
bounded history `VecDeque` (default capacity 256).

`IncidentSignalProjection`: key = `(EntityRef, SignalName)`. Stores
latest signal per key; supports the three query methods on
`IncidentSignalReadModel` (`current_signal`, `active_signals_for`,
`active_signals_for_incident_kind`).

### Acceptance criteria
- [ ] Heartbeat ring honours capacity
- [ ] `active_signals_for` returns only signals whose status is `Active`
- [ ] `active_signals_for_incident_kind` cross-references the contributing
      `IncidentSignalObservation` against `IncidentKind` (note: each
      signal observation must carry enough context to map to kind —
      verify with ADR-L1 §R1)

---

## BTH-25: `ReadModelStore` assembler + impl all six traits

**Type** Story • **Priority** High • **Estimate** M • **Component** read_models
**ADRs** R1, R3 • **Blocked by** BTH-21–24 • **Blocks** BTH-35

### Description
Add `src/read_models/store.rs` with the assembler:

```rust
pub struct ReadModelStore {
    pub state:           StateProjection,
    pub metric:          MetricProjection,
    pub health:          HealthProjection,
    pub capability:      CapabilityProjection,
    pub heartbeat:       HeartbeatProjection,
    pub incident_signal: IncidentSignalProjection,
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error("projection: {0}")] Projection(#[from] ProjectionError),
}

impl ReadModelStore {
    pub fn new(config: ReadModelStoreConfig) -> Self;
    pub fn apply(&mut self, obs: &Observation) -> Result<(), ApplyError>;
}

impl StateReadModel       for ReadModelStore { /* delegate to self.state */ }
impl MetricReadModel      for ReadModelStore { /* delegate to self.metric */ }
// … etc, six trait impls
```

`apply` dispatches by `ObservationPayload` variant; `Event`,
`Inventory`, `Transition`, `Diagnosis` are no-ops in V0.

### Acceptance criteria
- [ ] All six read-model traits implemented and delegate to fields
- [ ] `apply` dispatches per ADR-R1 §R1.4 / ADR-R3 §R3.3
- [ ] Integration test: feed a sequence of observations, query all
      read-model surfaces, verify correct values

---

# Phase 7 — Collector layer

## BTH-26: Collector traits + `BatchSink` + sidecar_id on context

**Type** Task • **Priority** High • **Estimate** M • **Component** collectors
**ADRs** C1, C3 §C3.1 • **Blocked by** BTH-1 • **Blocks** BTH-28, BTH-34

### Description
Define the trait surface in `src/collectors/traits.rs` and extend
`CollectionContext` with `sidecar_id`.

```rust
#[async_trait]
pub trait PollingCollector: Send + Sync {
    fn descriptor(&self) -> &CollectorDescriptor;
    async fn poll(&self, ctx: CollectionContext) -> ObservationBatch;
}

#[async_trait]
pub trait SubscriptionCollector: Send + Sync {
    fn descriptor(&self) -> &CollectorDescriptor;
    async fn run(&self, ctx: CollectionContext, sink: BatchSink)
        -> Result<(), CollectionError>;
}

pub struct BatchSink { /* mpsc::Sender<ObservationBatch> */ }
impl BatchSink {
    pub async fn send(&self, batch: ObservationBatch) -> Result<(), SinkError>;
}

pub enum SinkError { Closed }
```

Update `CollectionContext`:
```rust
pub struct CollectionContext {
    pub sidecar_id: SidecarId,           // NEW
    pub collector_id: CollectorId,
    pub target: CollectorTarget,
    pub now: DateTime<Utc>,
    pub run_id: CollectionRunId,
}
```

### Acceptance criteria
- [ ] Both traits compile, doc-commented
- [ ] `CollectionContext` has the new field
- [ ] Trait object compatibility verified (`Box<dyn PollingCollector>` compiles)
- [ ] Test that `BatchSink::send` returns `SinkError::Closed` after rx drop

---

## BTH-27: `BitcoinRpcClient` + `RpcError`

**Type** Task • **Priority** High • **Estimate** M • **Component** collectors
**ADRs** C3 §C3.8 • **Blocked by** BTH-1 • **Blocks** BTH-28

### Description
Thin in-crate JSON-RPC wrapper over `reqwest::Client` in
`src/collectors/bitcoin_core/rpc_client.rs`.

Method coverage:
- `get_blockchain_info() -> GetBlockchainInfoResponse`
- `get_mempool_info() -> GetMempoolInfoResponse`
- `get_network_info() -> GetNetworkInfoResponse`
- `get_peer_info() -> GetPeerInfoResponse`

Auth:
- `BitcoinRpcAuth::UserPass`: HTTP basic
- `BitcoinRpcAuth::CookieFile`: read file on each call (cache later)

`RpcError`: variants per ADR-C3 §C3.8.

Each call wrapped in `tokio::time::timeout(self.timeout, …)`.

### Acceptance criteria
- [ ] Four async methods, typed responses with serde
- [ ] `RpcError` carries enough info to map to `CollectionErrorKind`
- [ ] Integration test against a regtest Bitcoin Core (gated behind
      a `BITHOUND_TEST_REGTEST_URL` env var so CI can skip it locally)
- [ ] Timeout test: a slow mock server triggers `RpcError::Timeout`

---

## BTH-28: `BitcoinCoreRpcCollector`

**Type** Story • **Priority** High • **Estimate** L • **Component** collectors
**ADRs** C1, C2, C3 §§C3.2–C3.7 • **Blocked by** BTH-26, BTH-27 • **Blocks** BTH-36

### Description
First concrete `PollingCollector` impl in
`src/collectors/bitcoin_core/rpc.rs`.

Per poll:
1. Call `get_blockchain_info` → `BitcoinBlockchainState` observation +
   `bitcoin.rpc.getblockchaininfo` health Ok.
2. Same for mempool, network, peer.
3. On any RPC failure, return `ProbeResult::Failed` with the
   `HealthCheckObservation`, the `CollectionError`, and
   `partial_observations` accumulated so far.

```rust
pub struct BitcoinCoreRpcCollector {
    descriptor: CollectorDescriptor,
    client: BitcoinRpcClient,
}

impl BitcoinCoreRpcCollector {
    pub fn new(descriptor: CollectorDescriptor,
               connection: BitcoinNodeConnection,
               http: reqwest::Client,
               config: BitcoinCoreRpcCollectorConfig)
        -> Result<Self, BuildError>;
}

pub struct BitcoinCoreRpcCollectorConfig {
    pub timeout_per_rpc: Duration,    // default 5s
}
```

### Acceptance criteria
- [ ] `new` validates URL parses + auth shape, does NOT hit the network
- [ ] Successful poll returns `ProbeResult::Ok` with 4 state + 4 health observations
- [ ] First-RPC failure returns `Failed` with `partial_observations.len() == 0`
- [ ] Third-RPC failure returns `Failed` with `partial_observations.len() == 4`
      (state + health for the first two RPCs)
- [ ] `RpcError → CollectionErrorKind` mapping per ADR-C3 §C3.8 verified
- [ ] Integration test against regtest behind the env-var gate

---

# Phase 8 — Notifier sender implementations

## BTH-29: Implement `WebhookSender`

**Type** Task • **Priority** Medium • **Estimate** M • **Component** notifications
**ADRs** — (replaces existing stub) • **Blocked by** BTH-1 • **Blocks** —

### Description
Replace the stub in `src/notifications/targets/webhook/sender.rs` with
a real implementation. POST the rendered `WebhookPayload` JSON to
`WebhookTarget.url` with the configured `headers`. Map HTTP outcomes
to `DeliveryOutcome`:

- 2xx → `Delivered { external_ref: None }`
- 4xx (not 429) → `Permanent::BadRequest`
- 401/403 → `Permanent::AuthFailure`
- 410 → `Permanent::DestinationGone`
- 429 → `Transient::RateLimited` with `retry_after` parsed from header
- 5xx → `Transient::Upstream5xx`
- Network error → `Transient::Network`

### Acceptance criteria
- [ ] All seven outcome paths covered by tests (use `wiremock` or
      similar)
- [ ] Custom headers from `WebhookTarget.headers` are sent
- [ ] `SecretString` is never logged

---

## BTH-30: Implement `TelegramSender`

**Type** Task • **Priority** Medium • **Estimate** M • **Component** notifications
**ADRs** — • **Blocked by** BTH-1 • **Blocks** —

### Description
Replace the stub. Call `sendMessage` on the Telegram Bot API with the
`TelegramTarget.chat_id`, the rendered text, and the configured
`parse_mode`. Map outcomes:

- `ok: true` → `Delivered { external_ref: Some(Telegram{chat_id, message_id}) }`
- `error_code: 429` → `Transient::RateLimited` with `retry_after`
- `error_code: 401/403` → `Permanent::AuthFailure` or
  `Permanent::DestinationGone` (user blocked bot)
- Other 4xx → `Permanent::BadRequest`
- 5xx → `Transient::Upstream5xx`

Can use the existing `teloxide` dependency or hand-rolled HTTP — pick
whichever is smaller.

### Acceptance criteria
- [ ] Successful send returns `external_ref` with the real message_id
- [ ] Rate limit response surfaces `retry_after`
- [ ] All error variants exercised by tests

---

## BTH-31: Implement `DiscordSender`

**Type** Task • **Priority** Medium • **Estimate** M • **Component** notifications
**ADRs** — • **Blocked by** BTH-1 • **Blocks** —

### Description
Replace the stub. POST the rendered `DiscordPayload` JSON to the
webhook URL (`DiscordTarget.webhook_url`). Map outcomes per ADR-L5 §L5.4
spirit. On success, parse the response for `id` to populate
`external_ref`.

### Acceptance criteria
- [ ] Successful send returns `external_ref: ExternalMessageRef::Discord`
- [ ] Rate limit and 4xx/5xx paths exercised
- [ ] Allowed mentions default to none (already in render)

---

# Phase 9 — Config

## BTH-32: Config types + clap CLI

**Type** Task • **Priority** High • **Estimate** M • **Component** config
**ADRs** X1 §§X1.4, X1.9 • **Blocked by** BTH-1 • **Blocks** BTH-33

### Description
Add `src/config/` module tree (mod.rs + per-concern submodules per
ADR-X1 §X1.9). Define the `serde::Deserialize` types matching the V0
schema (§ X1.3). Define the `Cli` struct (`clap` derive).

```rust
// src/config/mod.rs
pub struct Config {
    pub sidecar: SidecarConfig,
    pub storage: StorageConfig,
    pub runtime: RuntimeConfig,
    pub incidents: IncidentsConfig,
    pub bitcoin_nodes: Vec<BitcoinNodeConfig>,
    pub collectors: Vec<CollectorDescriptorConfig>,
    pub notifications: NotificationsConfig,
    pub notification_rules: Vec<NotificationRuleConfig>,
}

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("io: {0}")] Io(#[from] std::io::Error),
    #[error("toml parse: {0}")] Toml(#[from] toml::de::Error),
    #[error("missing required env var: {0}")] MissingEnv(String),
    #[error("invalid: {0}")] Invalid(String),
    #[error("inline secret rejected at {0}")] InlineSecret(String),
}
```

### Acceptance criteria
- [ ] Types compile and deserialize a sample TOML (sample committed
      under `examples/bithound.example.toml`)
- [ ] `cli.rs` exposes `--config`, `--check-config`, `--version`
- [ ] Sample TOML covers all V0 sections from § X1.3 of the spec
- [ ] Parse failure surfaces a useful error pointing at the offending key

---

## BTH-33: TOML loading, env overrides, secrets resolution, validation

**Type** Story • **Priority** High • **Estimate** L • **Component** config
**ADRs** X1 §§X1.2, X1.5–X1.8 • **Blocked by** BTH-32 • **Blocks** BTH-36, BTH-37

### Description
Implement `Config::load_from_args_and_env()` per ADR-X1 §X1.8:

1. Parse CLI.
2. Resolve config path (`--config` → cwd → `/etc`).
3. Read + parse TOML.
4. Apply `BITHOUND_<SECTION>__<KEY>` env overrides (non-secret keys).
5. Validate shape + cross-references (collectors reference existing nodes).
6. Resolve `*_env` secrets into `SecretString` (existence check first).
7. Read or generate `SidecarId` via `sidecar.id_file`.
8. Open `SqlitePool` (run migrations).

Inline secret rejection: any field named `*_password`, `*_token`,
`*_secret` set to a non-`_env` value produces `InlineSecret`.

### Acceptance criteria
- [ ] All eight steps implemented; failure at any step exits with code 78
- [ ] Test: missing config file at all default paths fails cleanly
- [ ] Test: cross-reference error (collector targets nonexistent node)
- [ ] Test: env override changes a non-secret key
- [ ] Test: missing env var for a `_env`-suffixed field fails
- [ ] Test: inline `password = "foo"` returns `InlineSecret`
- [ ] Test: sidecar.id_file is generated on first run and reused thereafter
- [ ] `--check-config` prints the merged config with secrets redacted, exits 0

---

# Phase 10 — Runtime

## BTH-34: Collector supervisor module

**Type** Task • **Priority** High • **Estimate** M • **Component** runtime
**ADRs** S1, S3 §§S3.3–S3.4 • **Blocked by** BTH-26 • **Blocks** BTH-37

### Description
Add `src/runtime/supervisor.rs`. Spawns one tokio task per polling
collector and one per subscription collector. Polling tasks tick on
`tokio::time::interval(descriptor.integration.interval())`, call `poll`,
and send the batch through the mpsc channel.

Implements the respawn-with-backoff policy from ADR-S3 §S3.4:
`10s, 30s, 60s, 300s` cap; reset after 5 minutes of clean run.

Subscribes to a `broadcast::Receiver<()>` shutdown signal; selects
against it inside the loop to exit cleanly.

### Acceptance criteria
- [ ] Polling task ticks at the configured interval
- [ ] Crashed collector task is respawned per the backoff schedule
- [ ] Shutdown signal causes all collector tasks to exit within 5s
- [ ] Subscription collector `Err` return triggers respawn with backoff

---

## BTH-35: Pipeline consumer module

**Type** Story • **Priority** High • **Estimate** L • **Component** runtime
**ADRs** S1, S2, S3 §S3.8, **D4** • **Blocked by** BTH-11, BTH-12, BTH-19, BTH-25, BTH-26 • **Blocks** BTH-37

### Description

**Re-scoped by ADR-D4.** The consumer now pattern-matches the engine's
`Vec<IncidentEvent>` to perform side effects, instead of reading
`HandleOutcome` fields.

Add `src/runtime/consumer.rs`. The central consumer task per ADR-S1.

```rust
pub async fn run(
    mut rx: mpsc::Receiver<ObservationBatch>,
    rules: Vec<Box<dyn DiagnosticRule>>,
    mut read_models: ReadModelStore,
    mut engine: IncidentEngine,
    notifier: Notifier,
    observation_store: Arc<dyn ObservationStore>,
    incident_repo: Arc<dyn IncidentRepository>,
    mut shutdown: broadcast::Receiver<()>,
);
```

Per batch: append to observation store, apply to read models, build
`DiagnosticContext` from the batch's subject, evaluate every rule,
hand each (unvalidated) draft to the engine, then pattern-match the
returned `Vec<IncidentEvent>`:

```rust
for event in engine.handle(IncidentCommand::RecordSignal(draft), now)? {
    match event {
        IncidentEvent::SignalRecorded(obs)  => { observation_store.append(&obs).await?; read_models.apply(&obs)?; }
        IncidentEvent::IncidentTouched(inc) => incident_repo.save(&inc).await?,
        IncidentEvent::Lifecycle(ev)        => notifier.dispatch(&ev, &compose(&ev)).await,
        IncidentEvent::DraftRejected{…}     => tracing::warn!(…),
        IncidentEvent::DraftBelowConfidenceFloor{…} => tracing::debug!(…),
    }
}
```

Rule errors logged and skipped (ADR-S2). Repo write failures retried
with backoff per ADR-L4 §L4.4.

### Acceptance criteria
- [ ] Integration test feeds a batch, asserts: observation appended,
      read models updated, rules invoked, no incident events emitted
      (empty rules vec)
- [ ] With a stub rule that always emits a single Active draft, the
      consumer sees `[SignalRecorded, IncidentTouched, Lifecycle(Opened)]`
      in order; the notifier sees the Opened lifecycle event
- [ ] Rule panic isolated — next rule still runs
- [ ] Repo save failure on `IncidentTouched` triggers retry;
      exhaust = log + skip
- [ ] Shutdown drains remaining batches then exits
- [ ] Test asserts that events within a single command are processed
      in the order the engine emits them (per ADR-D4 event-ordering
      invariant)

---

## BTH-36: Bootstrap module — build collectors from config

**Type** Task • **Priority** High • **Estimate** M • **Component** runtime
**ADRs** S3 §§S3.7, C3 §C3.2 • **Blocked by** BTH-28, BTH-33 • **Blocks** BTH-37

### Description
Add `src/runtime/bootstrap.rs` with two functions:

```rust
pub fn build_polling_collectors(
    collector_configs: &[CollectorDescriptorConfig],
    registry: &NodeRegistry,
    http: &reqwest::Client,
) -> Result<Vec<Box<dyn PollingCollector>>, BuildError>;

pub fn build_subscription_collectors(
    collector_configs: &[CollectorDescriptorConfig],
    registry: &NodeRegistry,
    http: &reqwest::Client,
) -> Result<Vec<Box<dyn SubscriptionCollector>>, BuildError>;
```

V0: only `IntegrationKind::BitcoinCoreRpc` is handled; other variants
return `BuildError::NotImplemented` with a clear message.

### Acceptance criteria
- [ ] Build a `BitcoinCoreRpcCollector` from a descriptor that
      references an existing node
- [ ] Bad target reference returns `BuildError::TargetNotFound`
- [ ] Subscription variants return `BuildError::NotImplemented`

---

## BTH-37: `runtime::run()` + `main.rs` bootstrap

**Type** Story • **Priority** High • **Estimate** M • **Component** runtime
**ADRs** S1, S3 §§S3.3, S3.7 • **Blocked by** BTH-13, BTH-33, BTH-34, BTH-35, BTH-36, BTH-25 • **Blocks** BTH-38, BTH-40

### Description
Tie everything together.

`src/runtime/mod.rs`:
```rust
pub async fn run(deps: RuntimeDeps) -> Result<(), RuntimeError>;

pub struct RuntimeDeps {
    pub sidecar_id: SidecarId,
    pub polling_collectors: Vec<Box<dyn PollingCollector>>,
    pub subscription_collectors: Vec<Box<dyn SubscriptionCollector>>,
    pub rules: Vec<Box<dyn DiagnosticRule>>,
    pub read_models: ReadModelStore,
    pub engine: IncidentEngine,
    pub notifier: Notifier,
    pub observation_store: Arc<dyn ObservationStore>,
    pub incident_repo: Arc<dyn IncidentRepository>,
    pub config: RuntimeConfig,
}
```

`run` wires the mpsc channel, spawns the supervisor and consumer
tasks, awaits SIGINT/SIGTERM (ADR-S3 §S3.3), broadcasts shutdown,
joins with a 30s deadline.

`src/main.rs` becomes the bootstrap per ADR-S3 §S3.7.

### Acceptance criteria
- [ ] `cargo run -- --config examples/bithound.example.toml` starts
      and runs against a regtest node
- [ ] Sends SIGINT → clean shutdown within 5s
- [ ] No `println!` left in `main.rs`; tracing-subscriber set up
- [ ] Sidecar prints its `SidecarId` and the loaded collector descriptors
      at startup (info-level tracing)

---

# Phase 11 — First diagnostic rules

## BTH-38: `BitcoinTipLagRule` (catalog A1)

**Type** Story • **Priority** Medium • **Estimate** M • **Component** rules
**ADRs** L2 §L2.1 ; catalog A1 • **Blocked by** BTH-19, BTH-25, BTH-37 • **Blocks** BTH-40

### Description
Implement the first diagnostic rule. Triggers on the A1 pattern:
`initialblockdownload == true` AND `headers - blocks < 1000` AND
`verificationprogress > 0.999` AND `peer_count >= 8` AND tip time is
recent but `initialblockdownload` hasn't cleared.

Lives in `src/diagnostics/rules/bitcoin/tip_lag.rs`. Implements
`DiagnosticRule`. Reads from `ctx.state` (BitcoinBlockchain and
BitcoinPeerSummary) using the `StateReadModelExt` helpers.

Hysteresis (rule-owned per ADR-L2 §L2.1): require the condition to
hold across two consecutive observations.

### Acceptance criteria
- [ ] Active draft emitted when the A1 pattern is satisfied for 2 consecutive ticks
- [ ] Cleared draft emitted when any condition is broken for 2 consecutive ticks
- [ ] Test: single-tick A1 doesn't open an incident (debounce)
- [ ] Test: clearing path closes the incident with `Resolved`
- [ ] `kind` and `dimension` set per ADR-L1; subject = `EntityRef::BitcoinNode`

---

## BTH-39: `BitcoinPeerStarvationRule` (catalog A3)

**Type** Story • **Priority** Medium • **Estimate** M • **Component** rules
**ADRs** L2 §L2.1 ; catalog A3 • **Blocked by** BTH-19, BTH-25, BTH-37 • **Blocks** BTH-40

### Description
Triggers on A3: `getnetworkinfo.connections_out < 8` AND
`networkactive == true` AND condition has held for ≥ 30 minutes.
Severity escalates from Warning to Critical if tip is also stale
(no new blocks for 30+ minutes).

Lives in `src/diagnostics/rules/bitcoin/peer_starvation.rs`. Reads
BitcoinNetwork state + BitcoinPeerSummary + last-block timestamp from
BitcoinBlockchain state.

### Acceptance criteria
- [ ] Test: Warning draft after 30 min of low outbound count
- [ ] Test: severity escalates to Critical after 30 min of stale tip
- [ ] Test: cleared when outbound count recovers
- [ ] No drafts emitted when `networkactive == false` (operator disabled networking)

---

# Phase 12 — End-to-end & docs

## BTH-40: End-to-end integration test (regtest)

**Type** Story • **Priority** High • **Estimate** L • **Component** test
**ADRs** — • **Blocked by** BTH-37, BTH-38, BTH-39 • **Blocks** BTH-41

### Description
Integration test under `tests/` that:

1. Spawns a Bitcoin Core regtest node (via `bitcoind` binary or test
   docker image; document the prerequisite).
2. Writes a `bithound.toml` pointing at it.
3. Spawns `bithound` as a child process.
4. Forces a tip-lag condition (stop bitcoind for 13h is impractical;
   instead inject the condition via mocking the RPC client in a
   dedicated test path — OR use a regtest fixture that satisfies the
   IBD edge case).
5. Asserts that a webhook test endpoint receives an Opened event
   matching the A1 fingerprint.

Mark the test `#[ignore]` and gate it on `BITHOUND_TEST_REGTEST` env var
so CI doesn't run it locally without bitcoind.

### Acceptance criteria
- [ ] Test passes locally with bitcoind in PATH
- [ ] Failure produces a clear log of what was expected vs received
- [ ] Documented in `tests/README.md` how to run

---

## BTH-41: README + operator docs update

**Type** Task • **Priority** Medium • **Estimate** M • **Component** docs
**ADRs** — • **Blocked by** BTH-40 • **Blocks** —

### Description
Update `README.md` and add `docs/OPERATOR_GUIDE.md` covering:

- What V0 does (and what it doesn't).
- How to install (cargo install / docker / binary).
- How to write `bithound.toml`.
- How to set up Bitcoin Core for RPC access (user, password, cookie).
- How to set up Telegram / Discord / webhook notifications.
- How to interpret the two V0 diagnostic rules (link to catalog).
- Where logs and the DB file live.
- Troubleshooting (common config errors, exit code 78 meaning).

Also: rewrite the existing `docs/INCIDENT_CATALOG.md` cross-references
to point at the implemented rules (BTH-38, BTH-39).

### Acceptance criteria
- [ ] README has a "Quick start" section that works for a fresh user
- [ ] `OPERATOR_GUIDE.md` covers all sections above
- [ ] Catalog A1 and A3 entries link to their rule modules
- [ ] No "TBD" or "TODO" left in user-facing docs

---

# Phase D — Domain refinement (DMMF alignment)

Eight tickets aligning Bithound's type system with Wlaschin's
"Domain Modeling Made Functional" patterns. ADRs D1–D4.

## BTH-42: Smart-constructor scaffolding (`parse_dotted_name`)

**Type** Task • **Priority** High • **Estimate** S • **Component** shared
**ADRs** **D2** • **Blocked by** — • **Blocks** BTH-43, BTH-44, BTH-45, BTH-46

### Description

Add `src/shared/parse.rs` with the shared dotted-namespace parser and
error type. No newtypes migrated yet — that's BTH-43 onward.

```rust
// src/shared/parse.rs
pub fn parse_dotted_name(s: &str) -> Result<String, ParseDottedNameError>;

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ParseDottedNameError {
    #[error("name is empty")] Empty,
    #[error("name exceeds 128 characters (got {got})")] TooLong { got: usize },
    #[error("invalid character {found:?} at position {at}")] BadCharacter { at: usize, found: char },
    #[error("empty segment at position {at}")] EmptySegment { at: usize },
    #[error("segment at position {at} must start with a-z")] BadSegmentStart { at: usize },
    #[error("name must contain at least one dot")] NoDot,
}
```

### Acceptance criteria
- [ ] `parse_dotted_name` accepts `"bitcoin.tip_lag"`, `"lnd.channel.inactive"`, `"host.disk.exhaustion"`, `"sidecar.collector.run_started"`
- [ ] Rejects: `"tip_lag"` (NoDot), `"BitcoinTipLag"` (BadCharacter), `"bitcoin..tip_lag"` (EmptySegment), `"1bitcoin.x"` (BadSegmentStart), `"bitcoin.tip-lag"` (BadCharacter), `""` (Empty), 129-char string (TooLong)
- [ ] Each error variant exercised by a named unit test
- [ ] Doc-comment explains the grammar with examples

---

## BTH-43: Migrate `IncidentKind` to smart constructor

**Type** Task • **Priority** High • **Estimate** S • **Component** incidents
**ADRs** **D2** • **Blocked by** BTH-42 • **Blocks** BTH-47

### Description

Migrate `IncidentKind` from `pub struct IncidentKind(pub String)` to
the smart-constructor form. Update all call sites (well_known
references, tests).

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct IncidentKind(String);

impl IncidentKind {
    pub fn parse(s: impl AsRef<str>) -> Result<Self, ParseDottedNameError> { … }
    pub fn as_str(&self) -> &str { &self.0 }
    pub fn from_well_known(s: &'static str) -> Self { … }   // debug-asserts validity
}

impl AsRef<str> for IncidentKind { … }
impl std::fmt::Display for IncidentKind { … }
impl TryFrom<String> for IncidentKind { … }
impl From<IncidentKind> for String { … }
```

### Acceptance criteria
- [ ] Inner field is private
- [ ] `parse` returns `ParseDottedNameError` on invalid input
- [ ] `from_well_known` debug-asserts via `parse_dotted_name`; release builds skip the check
- [ ] Serde round-trips through string; deserialization re-validates (test asserts invalid JSON string fails)
- [ ] All existing call sites compile (`well_known` references migrated)
- [ ] Test: parity between `well_known::*` constants and `IncidentKind::from_well_known` for every canonical kind

---

## BTH-44: Migrate `MetricName` and `SignalName`

**Type** Task • **Priority** High • **Estimate** S • **Component** observations
**ADRs** **D2** • **Blocked by** BTH-42 • **Blocks** BTH-46

### Description

Same shape as BTH-43, applied to `MetricName` and `SignalName`. Both
gain `parse`, `as_str`, `AsRef<str>`, `Display`, serde round-trip with
re-validation, and `from_well_known` fast path.

### Acceptance criteria
- [ ] Same shape as BTH-43 for both newtypes
- [ ] Existing call sites compile
- [ ] Serde re-validation test per type

---

## BTH-45: Migrate `StateName` and `CapabilityName`

**Type** Task • **Priority** High • **Estimate** S • **Component** observations
**ADRs** **D2** • **Blocked by** BTH-42 • **Blocks** BTH-46

### Description

Same shape as BTH-43, applied to `StateName` and `CapabilityName`.
`StateName::from_well_known` is exercised by `StateObservation::name()`
(BTH-6 — already shipped) and the parity test in `state.rs`.

### Acceptance criteria
- [ ] Same shape as BTH-43 for both newtypes
- [ ] BTH-6's parity test still passes (now via `from_well_known`)
- [ ] Existing call sites compile

---

## BTH-46: Migrate remaining name newtypes; remove compat helpers

**Type** Task • **Priority** Medium • **Estimate** M • **Component** observations
**ADRs** **D2** • **Blocked by** BTH-43, BTH-44, BTH-45 • **Blocks** —

### Description

Migrate the remaining five name newtypes: `HealthTargetId`,
`EventName`, `TransitionName`, `InventoryName`, `DiagnosisName`.
Remove any compatibility helpers (e.g. `From<String>` on the public
constructor) added during BTH-42 — at this point all call sites have
migrated.

### Acceptance criteria
- [ ] All five remaining newtypes match the BTH-43 template
- [ ] No `pub` inner-field constructors remain for the ten name newtypes
- [ ] `cargo clippy -- -D warnings` clean

---

## BTH-47: Unvalidated/Validated draft split + `KindRegistry::validate`

**Type** Story • **Priority** High • **Estimate** M • **Component** diagnostics + incidents
**ADRs** **D1** • **Blocked by** BTH-4, BTH-15 • **Blocks** BTH-17 (re-scoped), BTH-19

### Description

Split `IncidentSignalDraft` into two distinct structs per ADR-D1:

```rust
// src/diagnostics/types.rs
pub struct UnvalidatedIncidentSignalDraft {
    pub subject: EntityRef, pub signal: SignalName,
    pub kind: IncidentKind, pub dimension: Option<String>,
    pub severity: SignalSeverity, pub status: SignalStatus,
    pub confidence: Confidence, pub evidence: Vec<EvidenceRef>,
}

// src/incidents/kinds.rs
pub struct ValidatedIncidentSignalDraft {
    // same fields, but PRIVATE; constructed only via KindRegistry::validate
    subject: EntityRef, signal: SignalName,
    kind: IncidentKind, dimension: Option<String>,
    severity: SignalSeverity, status: SignalStatus,
    confidence: Confidence, evidence: Vec<EvidenceRef>,
}

impl ValidatedIncidentSignalDraft {
    pub fn subject(&self) -> &EntityRef { … }
    pub fn kind(&self) -> &IncidentKind { … }
    // … per-field accessors
}

impl KindRegistry {
    pub fn validate(&self, draft: UnvalidatedIncidentSignalDraft)
        -> Result<ValidatedIncidentSignalDraft, DraftError>;
}
```

Update `DiagnosticRule::evaluate` to return
`Vec<UnvalidatedIncidentSignalDraft>`.

### Acceptance criteria
- [ ] Both structs defined; validated form has private fields and accessor methods only
- [ ] `KindRegistry::validate` is the only public way to construct a `ValidatedIncidentSignalDraft`
- [ ] `UnvalidatedIncidentSignalDraft` is `Serialize + Deserialize`
- [ ] `ValidatedIncidentSignalDraft` is `Serialize` but **not** `Deserialize` (test asserts the trait isn't implemented)
- [ ] `DiagnosticRule::evaluate` signature updated; existing rules (none yet) would compile
- [ ] Test matrix: validate succeeds for valid drafts; returns `DraftError::UnknownKind`, `DisallowedSubject`, `DimensionRequired`, `DimensionForbidden` as appropriate

---

## BTH-48: Promote `ActorId`; extend `IncidentCommand` with stubs

**Type** Task • **Priority** Medium • **Estimate** S • **Component** incidents + shared
**ADRs** **D3** • **Blocked by** BTH-2 • **Blocks** BTH-17 (re-scoped)

### Description

1. Add `ActorId` to `src/shared/types.rs`:
   ```rust
   pub struct ActorId(pub String);
   impl ActorId {
       pub fn system() -> Self { Self("system".into()) }
       pub fn operator(name: impl Into<String>) -> Self { Self(name.into()) }
   }
   ```
2. Extend `IncidentCommand` per ADR-D3:
   ```rust
   pub enum IncidentCommand {
       RecordSignal(UnvalidatedIncidentSignalDraft),
       Acknowledge { id: IncidentId, by: ActorId, at: DateTime<Utc> },
       Resolve     { id: IncidentId, by: ActorId, at: DateTime<Utc>, reason: String },
   }
   ```
3. Extend `EngineError` with `NotYetImplemented(&'static str)` variant.

### Acceptance criteria
- [ ] `ActorId` in `src/shared/types.rs` with `system()` and `operator()`
- [ ] `IncidentCommand` has three variants
- [ ] `EngineError::NotYetImplemented` defined
- [ ] Serde round-trip test for `ActorId`

---

## BTH-49: `SuppressionCommand` + `SuppressionService` trait

**Type** Task • **Priority** Low • **Estimate** S • **Component** incidents
**ADRs** **D3**, L5 • **Blocked by** BTH-48 • **Blocks** —

### Description

Add the separate suppression command vocabulary in
`src/incidents/suppression.rs`:

```rust
pub enum SuppressionCommand {
    Suppress {
        fingerprint: IncidentFingerprint,
        until: Option<DateTime<Utc>>,
        by: ActorId,
        reason: String,
    },
    Unsuppress { fingerprint: IncidentFingerprint, by: ActorId },
}

#[async_trait]
pub trait SuppressionService: Send + Sync {
    async fn handle(&self, cmd: SuppressionCommand, now: DateTime<Utc>)
        -> Result<(), SuppressionError>;
}

pub enum SuppressionError {
    NotYetImplemented(&'static str),
    Repository(RepoError),
}
```

V0/V0.1 ships no concrete `SuppressionService` impl; the trait stub is
forward-compat for V0.2.

### Acceptance criteria
- [ ] Both types + trait defined and doc-commented
- [ ] `SuppressionError` derives `Debug`
- [ ] No concrete impl required (this ticket is types-only)

---

## BTH-50: Per-context `events.rs` modules + `DomainEvent` envelope

**Type** Story • **Priority** Medium • **Estimate** M • **Component** all domain contexts
**ADRs** **D4** • **Blocked by** BTH-5, BTH-17 (re-scoped) • **Blocks** —

### Description

Add per-context events modules per ADR-D4:

- `src/observations/events.rs` — `ObservationEvent::{BatchProduced, ObservationAppended}`.
- `src/read_models/events.rs` — `ReadModelEvent::Applied`.
- `src/diagnostics/events.rs` — `DiagnosticEvent::{DraftEmitted, RuleFailed}`.
- `src/incidents/events.rs` — `IncidentEvent` (already defined by BTH-17; this ticket just declares the module is present and exported).
- `src/notifications/events.rs` — `NotificationEvent::{Dispatched, Suppressed}`.

Plus the top-level envelope:

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

V0 doesn't dispatch on an event bus — the enums are type-level
documentation for what crosses context boundaries, used for tracing
and (later) cloud sync.

### Acceptance criteria
- [ ] All five per-context events modules present and `pub`
- [ ] Top-level `DomainEvent` envelope defined with `From<T>` impls for each variant
- [ ] All events derive `Debug + Clone + Serialize` (cloud-sync-ready); `Deserialize` for round-trip
- [ ] `cargo doc --no-deps` lists each events module
- [ ] No runtime dispatch wired in this ticket — observation-only types

---

# Phase 3 (continued) — Notification attempts persistence

Per ADR-P3. Three new tickets and one re-scope on BTH-13.

## BTH-51: `NotificationAttemptRepository` trait + revised `NotificationAttempt` + memory impl

**Type** Task • **Priority** High • **Estimate** S • **Component** notifications + storage
**ADRs** **P3** (§§P3.3, P3.4) • **Blocked by** #1 • **Blocks** #64 (BTH-52), #65 (BTH-53)

### Description

Add `src/notifications/repository.rs` with the trait and revise the existing `NotificationAttempt` struct to carry retry state.

```rust
// src/notifications/repository.rs
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

Revise `NotificationAttempt`:

- Remove `incident_lifecycle_event: IncidentLifecycleEvent` (duplicates incidents rows).
- Remove `target: NotificationTarget` (carries `SecretString` — must not be persisted).
- Add `incident_id: IncidentId`, `lifecycle_kind: IncidentNotificationEventKind`.
- Add `target_kind: TargetKind` (enum: Telegram/Discord/Webhook/Stdout) and `target_summary: String` (redacted).
- Add `attempt_number: u32`, `parent_attempt_id: Option<NotificationAttemptId>`, `next_retry_at: Option<DateTime<Utc>>`.
- Replace `error: Option<String>` with `outcome: Option<DeliveryOutcome>` + `external_ref: Option<ExternalMessageRef>`.

Expand `NotificationDeliveryStatus`: `Pending`, `Succeeded`, `FailedTransient`, `FailedPermanent`, `Suppressed`.

Add `MemoryNotificationAttemptRepository` (test impl) under `src/storage/memory/`.

### Acceptance criteria

- [ ] Trait + revised struct + `TargetKind` enum compile with doc-comments
- [ ] `MemoryNotificationAttemptRepository` implements the trait
- [ ] Trait conformance test suite shared with the SQLite impl
- [ ] Round-trip test: insert pending, complete, list_retryable returns the row when `next_retry_at <= now`, doesn't return it when `now < next_retry_at`
- [ ] `target_summary` redaction sketches: `telegram:chat_id=N`, `discord:webhook=host=H`, `webhook:host=H`, `stdout`

---

## BTH-52: `SqliteNotificationAttemptRepository` + schema amendment

**Type** Task • **Priority** High • **Estimate** M • **Component** storage
**ADRs** **P3** (§§P3.2, P3.3) • **Blocked by** #9 (BTH-9), #63 (BTH-51) • **Blocks** #65 (BTH-53)

### Description

Amend `migrations/0001_initial.sql` with the `notification_attempts` table (per ADR-P3 §P3.2 DDL) and add `SqliteNotificationAttemptRepository` in `src/storage/sqlite/notification_attempt_repository.rs`.

Implement:
- `insert_pending`: INSERT with all retry-state columns.
- `complete`: UPDATE the row from `Pending` to a terminal status; set `next_retry_at` per the passed value.
- `list_retryable`: SELECT WHERE `status = 'FailedTransient' AND next_retry_at <= ?` ORDER BY `next_retry_at` LIMIT `?`.
- `list_for_incident`: SELECT WHERE `incident_id = ?` ORDER BY `attempted_at DESC`.

### Acceptance criteria

- [ ] `notification_attempts` table created with all columns + 4 indexes per ADR-P3 §P3.2
- [ ] `STRICT` keyword present
- [ ] Trait conformance suite (from BTH-51) passes
- [ ] Round-trip test for every `DeliveryOutcome` variant via `outcome_json`
- [ ] `external_ref_json` round-trips through `ExternalMessageRef` for Telegram + Discord
- [ ] Test: `list_retryable` is index-using (check with `EXPLAIN QUERY PLAN`)
- [ ] Migration is idempotent (re-running the migrate step doesn't fail)

---

## BTH-53: Retry scheduler + `Notifier::dispatch` signature change + backoff defaults

**Type** Story • **Priority** High • **Estimate** M • **Component** runtime + notifications
**ADRs** **P3** (§§P3.5, P3.6, P3.7, P3.8, P3.9, P3.10) • **Blocked by** #63 (BTH-51), #64 (BTH-52), #35 (BTH-35) • **Blocks** —

### Description

Three coordinated changes:

1. **`Notifier::dispatch` signature change** (ADR-P3 §P3.10):
   ```rust
   pub async fn dispatch(
       &self,
       event: &IncidentLifecycleEvent,
       message: &NotificationMessage,
       attempts_repo: &dyn NotificationAttemptRepository,
       now: DateTime<Utc>,
   ) -> Vec<NotificationAttempt>;
   ```
   Internally: for each matching rule, INSERT `Pending`, call sender, UPDATE with receipt + `next_retry_at`.

2. **Retry scheduler tick in `runtime::consumer`** (ADR-P3 §P3.7): add a `tokio::time::interval` (10s default, configurable via `RuntimeConfig::notification_retry_tick_seconds`) as a third `select!` arm. On tick: `list_retryable`, reconstruct event from incident + lifecycle_kind, re-render message, call `notifier.retry_one`.

3. **Backoff defaults per target kind** (ADR-P3 §P3.5): when the protocol surfaces a `retry_after`, honor it; otherwise use `[30s, 120s, 600s]` for Telegram/Discord/Webhook and no-retry for Stdout. Max 3 retries (4 total attempts) — configurable via `RuntimeConfig::notification_max_retries`.

### Acceptance criteria

- [ ] `Notifier::dispatch` matches the new signature
- [ ] Initial dispatch inserts `Pending` and updates to terminal status in one logical sequence
- [ ] Transient outcome with retries remaining sets `next_retry_at` and status `FailedTransient`
- [ ] Transient outcome with retries exhausted sets status `FailedPermanent` with `outcome_kind = 'Transient'`
- [ ] Permanent / Delivered / Suppressed outcomes set terminal status with `next_retry_at = NULL`
- [ ] Suppressed deliveries (when ADR-L5 suppression is wired) get recorded with `status = 'Suppressed'`
- [ ] Retry tick picks up retryable rows, inserts new row with `attempt_number + 1` and `parent_attempt_id` set
- [ ] Message is re-rendered from current incident state at retry time (per ADR-P3 §P3.8) — test asserts that an incident updated between attempts produces a different rendered title/summary
- [ ] Protocol-supplied `retry_after` overrides default backoff (test with mock Telegram response carrying `parameters.retry_after = 60`)
- [ ] Stdout target never retries
- [ ] Consumer task shutdown drains the retry tick cleanly

---

## Re-scope: BTH-13 (retention task)

**Re-scoped by ADR-P3 §P3.11.**

`RetentionConfig` gains a fourth knob:

```rust
pub struct RetentionConfig {
    pub observations_max_age:  Option<Duration>,
    pub incidents_max_age:     Option<Duration>,
    pub suppressions_grace:    Option<Duration>,
    pub attempts_max_age:      Option<Duration>,   // NEW (default 30 days)
    pub vacuum_interval:       Duration,
}
```

The sweep gains a fourth `DELETE`:

```sql
DELETE FROM notification_attempts
WHERE attempted_at < ? AND status != 'Pending';
```

`status != 'Pending'` guards against deleting an in-flight attempt. Updated acceptance criteria on BTH-13:

- [ ] `RetentionConfig` has the new field with default `Some(Duration::from_days(30))`
- [ ] Test: completed attempts older than `attempts_max_age` are deleted
- [ ] Test: `Pending` rows are never deleted, even if older than the threshold

---

# Phase A — Local operator API (post-architecture-review)

Per ADR-A1. The operator-facing HTTP surface that closes the V0 product loop.

## BTH-56: axum HTTP server bootstrap + graceful shutdown

**Type** Task • **Priority** High • **Estimate** S • **Component** api
**ADRs** **A1** • **Blocked by** #1 (BTH-1, deps) • **Blocks** #70 (BTH-57)

### Description

Add `src/api/` module tree with `axum`-based HTTP server bootstrap. The server is a third tokio task spawned alongside the consumer and the notification worker (per ADR-A1 §A1.4). Binds `127.0.0.1:8487` by default, configurable via `bithound.toml [api]`.

```toml
[api]
bind = "127.0.0.1:8487"
enabled = true
```

```rust
// src/api/server.rs
pub async fn run(
    bind: SocketAddr,
    deps: ApiDeps,
    mut shutdown: broadcast::Receiver<()>,
) -> Result<(), ApiError>;

pub struct ApiDeps {
    pub sidecar_id: SidecarId,
    pub started_at: DateTime<Utc>,
    pub incident_repo: Arc<dyn IncidentRepository>,
    pub observation_store: Arc<dyn ObservationStore>,
    pub attempts_repo: Arc<dyn NotificationAttemptRepository>,
}
```

Add dependencies to `Cargo.toml`:
```toml
axum = "0.7"
tower = "0.5"
tower-http = { version = "0.6", features = ["trace"] }
```

No handlers yet — that's BTH-57.

### Acceptance criteria
- [ ] `cargo run -- --config examples/bithound.example.toml` starts an HTTP server on `127.0.0.1:8487`
- [ ] `GET /` returns 404 (no routes mounted yet)
- [ ] SIGINT to the process triggers graceful shutdown of the server within 5s
- [ ] `[api].enabled = false` disables the server entirely (no bind)
- [ ] `[api].bind` accepts and validates `SocketAddr` strings
- [ ] tower-http tracing middleware logs each request at info-level

---

## BTH-57: V0 operator API endpoints

**Type** Story • **Priority** High • **Estimate** M • **Component** api
**ADRs** **A1** §A1.1 • **Blocked by** #69 (BTH-56), #12 (BTH-12), #11 (BTH-11), #64 (BTH-52) • **Blocks** —

### Description

Implement four read-only endpoints per ADR-A1 §A1.1:

```text
GET /health                    — sidecar liveness + DB check + collector status
GET /incidents/open            — incidents with status != Resolved
GET /incidents/:id             — full incident detail
GET /incidents/:id/evidence    — full observations referenced by incident.evidence
```

JSON DTOs in `src/api/dto.rs` separate from domain types so the wire format can evolve independently.

Handlers under `src/api/handlers/`:
- `health.rs`
- `incidents.rs` (three routes for incidents)

Error handling: `ApiError` implements `IntoResponse`. 404 for missing IDs, 503 for DB unreachability on `/health`, 500 for unexpected errors.

### Acceptance criteria
- [ ] `curl localhost:8487/health` returns 200 with the documented JSON shape
- [ ] `curl localhost:8487/incidents/open` returns 200 with an array (empty when no incidents)
- [ ] `curl localhost:8487/incidents/:id` returns 200 with the full Incident or 404
- [ ] `curl localhost:8487/incidents/:id/evidence` returns the referenced observations; missing observations (swept by retention) are silently omitted
- [ ] DTOs round-trip via serde (parse the JSON back into the DTO struct)
- [ ] When DB is unreachable, `/health` returns 503 with `db.reachable=false` in the body
- [ ] Integration test using `axum::Router::oneshot()` covers all four routes

---

# Phase 1 (continued) — Identity refinements

## BTH-54: `EntityRef::Sidecar` + sub-entity scoping (ADR-N1)

**Type** Task • **Priority** High • **Estimate** S • **Component** shared
**ADRs** **N1** • **Blocked by** #3 (BTH-3, `EntitySubjectKind`) • **Blocks** #54 (BTH-47 ADR-D1 — and any downstream that uses EntityRef in identity-sensitive ways)

### Description

Two changes to `src/shared/types.rs`:

1. Add `EntityRef::Sidecar(SidecarId)` variant and the matching `EntitySubjectKind::Sidecar` discriminant.

2. Scope sub-entity IDs under their parent node:

```rust
pub enum EntityRef {
    Sidecar(SidecarId),
    Host(HostId),
    BitcoinNode(BitcoinNodeId),
    BitcoinPeer { node_id: BitcoinNodeId, peer_id: BitcoinPeerId },
    LndNode(LndNodeId),
    LndPeer    { node_id: LndNodeId, peer_id: LndPeerId },
    LndChannel { node_id: LndNodeId, channel_id: LndChannelId },
    LndInvoice { node_id: LndNodeId, invoice_id: LndInvoiceId },
}
```

Update:
- `EntityRef::subject_kind()` exhaustive match (compile error if a variant is added without updating).
- `IncidentFingerprint::as_key` (and its `subject_kind_and_id` helper) to produce `parent_id/sub_id` form for scoped variants and `sidecar` for the new variant.

### Acceptance criteria
- [ ] `EntityRef` has the new `Sidecar` variant and four scoped variants
- [ ] `EntitySubjectKind::Sidecar` added
- [ ] `subject_kind()` covers all 8 variants exhaustively (compile error if any new variant is added)
- [ ] `IncidentFingerprint::as_key` produces `sidecar|<id>|<kind>|-` for sidecar subjects
- [ ] `as_key` produces `lnd_channel|<node_id>/<channel_id>|<kind>|-` for scoped variants
- [ ] Serde round-trip tests for the new variants
- [ ] Existing fingerprint tests (BTH-4) still pass against the scoped form

---

# Phase 10 (continued) — Notification worker

## BTH-55: Notification worker task (ADR-N2)

**Type** Story • **Priority** High • **Estimate** M • **Component** runtime + notifications
**ADRs** **N2** (amends S1 §S1.4, P3 §P3.7) • **Blocked by** #63 (BTH-51), #35 (BTH-35 re-scoped) • **Blocks** —

### Description

Move notification dispatch out of the central consumer task into a separate worker task per ADR-N2.

Changes:

1. **Consumer task** (BTH-35 re-scope): on `IncidentEvent::Lifecycle`, the consumer:
   - composes the `NotificationMessage`
   - for each matching `NotificationRule`: builds a `NotificationAttempt` (status=Pending), calls `attempts_repo.insert_pending()`
   - sends a `NotificationDispatch { event, message, attempts, targets }` over an mpsc channel to the worker

2. **New notification worker task** in `src/runtime/notification_worker.rs`:
   - receives `NotificationDispatch` messages
   - for each `(attempt_id, target)`: calls the sender, builds `DeliveryReceipt`, calls `attempts_repo.complete(attempt_id, receipt, None)` (no retry path in V0)

3. **Channel**: bounded `mpsc::channel<NotificationDispatch>(256)`. Backpressure on the consumer's `send` is acceptable.

4. **Supervisor**: spawns the worker alongside the consumer; subscribes to the shared broadcast shutdown signal.

The V0 worker has no retry tick (per ADR-P3 audit-only revision and ADR-N2 §N2.4). V0.1 (BTH-53) adds the retry `select!` arm.

### Acceptance criteria
- [ ] `NotificationDispatch` channel type defined
- [ ] Consumer no longer calls `notifier.dispatch()` directly
- [ ] Worker task in `src/runtime/notification_worker.rs` with `run(rx, attempts_repo, senders, shutdown)`
- [ ] Pending row inserted by the consumer before the event is sent to the worker
- [ ] Worker UPDATEs the row to terminal status after dispatch completes
- [ ] If the worker is killed mid-dispatch, the row stays Pending (audit trail preserved)
- [ ] Two-writer invariant: consumer INSERTs Pending rows; worker UPDATEs to terminal. Per-row immutability preserved (each row has one INSERT + one UPDATE).
- [ ] Integration test: shutdown signal stops both tasks within 5s

---

# Phase 11 (continued) — RPC unreachability rule

## BTH-58: `BitcoinRpcUnreachableRule` + `bitcoin.rpc_unreachable` kind

**Type** Story • **Priority** Medium • **Estimate** S • **Component** rules
**ADRs** L2 §L2.1; review §3 V0 rules • **Blocked by** #19 (BTH-19), #25 (BTH-25), #37 (BTH-37), #16 (BTH-16) • **Blocks** #40 (BTH-40)

### Description

Add the third V0 rule (per the architecture review §3): `bitcoin.rpc_unreachable`. Fires when Bithound cannot reach the node's RPC over a sustained interval — i.e. the operator can't query their node.

Lives in `src/diagnostics/rules/bitcoin/rpc_unreachable.rs`. Implements `DiagnosticRule`. Reads from `ctx.health` (via `HealthReadModel::current_health`) for the four Bitcoin RPC health targets:

- `bitcoin.rpc.getblockchaininfo`
- `bitcoin.rpc.getmempoolinfo`
- `bitcoin.rpc.getnetworkinfo`
- `bitcoin.rpc.getpeerinfo`

Pattern: Active if all four health observations show `HealthStatus::Critical` for ≥ 60 seconds. Cleared if any returns to `HealthStatus::Ok`.

Also add the kind to `config/default_kinds.toml` (per BTH-16):

```toml
[[kinds]]
name = "bitcoin.rpc_unreachable"
allowed_subjects = ["BitcoinNode"]
allows_dimension = false
min_open_confidence = "High"
```

`min_open_confidence = "High"` because RPC unreachability is unambiguous when all four targets are simultaneously critical.

### Acceptance criteria
- [ ] Rule emits Active draft when all four RPC health targets are Critical for ≥ 60s
- [ ] Rule emits Cleared draft when any target returns to Ok
- [ ] Confidence = High; severity = Critical
- [ ] `kind = IncidentKind::from_well_known("bitcoin.rpc_unreachable")`
- [ ] Subject = `EntityRef::BitcoinNode(node_id)`
- [ ] Tests covering: all-four-critical → Active; partial-recovery → Cleared; brief outage (<60s) → no draft
- [ ] `default_kinds.toml` entry added with parity-test pass

---

# Phase V0.8 — LND foundation

## BTH-59: Vendor LND `.proto` files + add `tonic` / `prost` / `tonic-build` deps + `build.rs`

**Type** Task • **Priority** High • **Estimate** L • **Component** build + collectors
**ADRs** E2 §E2.1 §E2.2 • **Blocked by** — • **Blocks** BTH-62

### Description

First gRPC client in the codebase. Vendor LND's `.proto` files under `src/collectors/lnd/proto/` at a pinned version (initial target: **LND v0.18.5-beta**), add the tonic build chain, and stand up the empty `src/collectors/lnd/` module.

Per ADR-E2 §E2.2 this is **not a one-file copy**. LND's `lightning.proto` imports `google/api/annotations.proto` and `google/api/http.proto` (plus a few sub-service protos); all transitive imports must be vendored under `proto/google/api/` (recommended path) or stripped from a local copy (alternative — creates drift). Budget 2-3 days for chasing imports and validating codegen on the first vendor PR.

Cargo deps added:

```toml
[build-dependencies]
tonic-build = "0.12"

[dependencies]
tonic = { version = "0.12", features = ["tls"] }
prost = "0.13"
rustls-pemfile = "2"
```

`build.rs` at the repo root:

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=src/collectors/lnd/proto/lightning.proto");
    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .compile_protos(
            &["src/collectors/lnd/proto/lightning.proto"],
            &["src/collectors/lnd/proto"],
        )?;
    Ok(())
}
```

Vendored proto files include an MIT/upstream-license attribution preserved verbatim (LND's license header). `proto/README.md` documents the pinned LND version, the source URL, the vendored set, and the review-on-LND-release cadence.

### Acceptance criteria
- [ ] `lightning.proto` and transitive `google/api/*` protos vendored under `src/collectors/lnd/proto/`
- [ ] `proto/README.md` documents pinned LND version + source URL + update cadence
- [ ] LND license preserved (MIT) and attribution noted
- [ ] `tonic`, `prost`, `rustls-pemfile`, `tonic-build` added to `Cargo.toml` (no `default-features = false` on tonic)
- [ ] `build.rs` compiles the protos with `cargo:rerun-if-changed`
- [ ] `cargo build` succeeds; generated `lnrpc::*` types are importable via `tonic::include_proto!("lnrpc")`
- [ ] `src/collectors/lnd/mod.rs` exists as `//! LND collectors (gRPC-based).` placeholder; `pub mod lnd;` added to `src/collectors/mod.rs`
- [ ] No public API change vs main except the new `collectors::lnd` module

---

## BTH-60: `StateObservation::LndChannel(LndChannelState)` + `lnd.channel_detail` state constant + parity tests

**Type** Story • **Priority** High • **Estimate** S • **Component** observations
**ADRs** E1 §E1.1 (with E2 amendment for `peer_online`) • **Blocked by** — • **Blocks** BTH-63, BTH-64

### Description

Per ADR-E1 §E1.1, add the per-channel state surface the B1 rule (BTH-64) consumes. The struct lives in `src/observations/types/state.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LndChannelState {
    pub remote_pubkey: String,
    pub capacity_sat: u64,
    pub local_balance_sat: u64,
    pub remote_balance_sat: u64,
    /// Mirror of LND's `lnrpc.Channel.active`. B1 reads as-is.
    pub active: bool,
    pub private: bool,
    pub initiator: bool,
    pub csv_delay: u32,
    pub commit_fee_sat: u64,
    pub lifetime_seconds: u64,
    pub last_update_height: Option<u64>,
    /// SCID once gossip-eligible. Informational; identity is the funding outpoint.
    pub short_channel_id: Option<String>,
    /// Derived in the collector (BTH-63) by cross-referencing the channel's
    /// `remote_pubkey` against the same poll tick's `ListPeers` response.
    /// `None` when the cross-reference can't be made (e.g. ListPeers failed).
    pub peer_online: Option<bool>,
}
```

Add `StateObservation::LndChannel(LndChannelState)` enum variant, update `StateObservation::name()` to return `well_known::LND_CHANNEL_DETAIL`, add `LND_CHANNEL_DETAIL: &str = "lnd.channel_detail"` to `src/observations/types/state/well_known.rs` and to `ALL`. **Channel subject is `EntityRef::LndChannel { node_id, channel_id }`**; `LndChannelId` is the funding outpoint (`"txid:vout"`) for the channel's whole lifecycle (NOT the SCID).

### Acceptance criteria
- [ ] `LndChannelState` struct added with all fields above
- [ ] `StateObservation::LndChannel(LndChannelState)` variant added
- [ ] `StateObservation::name()` returns `LND_CHANNEL_DETAIL` for the variant
- [ ] `well_known::LND_CHANNEL_DETAIL` constant added and included in `state::well_known::ALL`
- [ ] Existing parity test `state.rs::parity_variants_match_well_known` passes
- [ ] Serde round-trip test for the new variant
- [ ] Doc comments preserve the funding-outpoint-as-identity rationale and the `peer_online` semantics

---

## BTH-61: `lnd.channel_inactive` + `lnd.chain_backend_lag` incident kinds + `default_kinds.toml` entries + bidirectional parity test

**Type** Story • **Priority** High • **Estimate** S • **Component** incidents
**ADRs** E1 §E1.4 §E1.5 • **Blocked by** — • **Blocks** BTH-64, BTH-65, BTH-66

### Description

Per ADR-E1 §E1.4 §E1.5, register the two V0.8 LND incident kinds.

`src/incidents/well_known.rs`:

```rust
pub const LND_CHANNEL_INACTIVE: &str = "lnd.channel_inactive";
pub const LND_CHAIN_BACKEND_LAG: &str = "lnd.chain_backend_lag";

pub const ALL: &[&str] = &[
    BITCOIN_RPC_UNREACHABLE,
    BITCOIN_NO_PEERS,
    BITCOIN_TIP_LAG_OR_IBD_STALLED,
    LND_CHANNEL_INACTIVE,
    LND_CHAIN_BACKEND_LAG,
];
```

`config/default_kinds.toml` entries:

```toml
[[kinds]]
name = "lnd.channel_inactive"
allowed_subjects = ["LndChannel"]
allows_dimension = false
min_open_confidence = "Medium"

[[kinds]]
name = "lnd.chain_backend_lag"
allowed_subjects = ["LndNode"]
allows_dimension = false
min_open_confidence = "High"
```

Also lands the **bidirectional parity test** named in ADR-E1 §E1.4: in addition to the existing `embedded_default_kinds_match_well_known_constants` (constants → registry), add an `embedded_default_kinds_subset_of_well_known_constants` test (registry → constants) so a contributor adding a TOML entry without updating `ALL` fails the build.

### Acceptance criteria
- [ ] `LND_CHANNEL_INACTIVE` and `LND_CHAIN_BACKEND_LAG` constants added and in `ALL`
- [ ] `default_kinds.toml` entries land with `allowed_subjects` + `allows_dimension` + `min_open_confidence` per ADR-E1 §E1.5
- [ ] Existing parity test (`constants → TOML`) still passes
- [ ] **New** parity test (`TOML → constants`) added and passes
- [ ] `IncidentKind::from_well_known(LND_CHANNEL_INACTIVE)` and `..._CHAIN_BACKEND_LAG` round-trip via the registry

---

# Phase V0.8 — LND client + collector

## BTH-62: `LndGrpcClient` — tonic + macaroon header + LND-cert-only TLS + per-RPC timeout + error mapping

**Type** Story • **Priority** High • **Estimate** M • **Component** collectors
**ADRs** E2 §E2.3 §E2.6 §E2.7 §E2.8 §E2.9 • **Blocked by** BTH-59 • **Blocks** BTH-63

### Description

Implement `src/collectors/lnd/grpc_client.rs` per ADR-E2 §E2.3. Thin tonic wrapper, one per LND node. Consumes the existing `LndNodeConnection` from `src/collectors/registry.rs` (the macaroon arrives pre-resolved as `SecretString`; only the TLS cert is loaded from disk).

```rust
pub struct LndGrpcClient {
    channel: tonic::transport::Channel,
    macaroon_hex: String,
    timeout: Duration,
}

pub enum BuildError {
    TlsCertRead { path: String, source: std::io::Error },
    TlsCertParse { path: String },
    MissingScheme { endpoint: String },
    InvalidEndpoint { endpoint: String, source: tonic::transport::Error },
    TlsConfig(tonic::transport::Error),
    MacaroonInvalid,
}

impl LndGrpcClient {
    pub fn new(
        endpoint: String,
        tls_cert_path: String,
        macaroon: &SecretString,
        timeout: Duration,
    ) -> Result<Self, BuildError>;

    pub async fn get_info(&self) -> Result<GetInfoResponse, LndRpcError>;
    pub async fn list_channels(&self) -> Result<ListChannelsResponse, LndRpcError>;
    pub async fn list_peers(&self) -> Result<ListPeersResponse, LndRpcError>;
}
```

**TLS:** load the configured LND cert via `rustls-pemfile`, build `ClientTlsConfig::new().ca_certificate(...)`. **Do NOT** enable tonic's `tls-roots` / `tls-webpki-roots` features. **No native roots.** Endpoint must include `https://` scheme or `BuildError::MissingScheme`.

**Macaroon:** hex-encode at construction; attach as `macaroon` metadata header per-request (no interceptor for V0.8).

**Timeout:** each RPC wrapped in `tokio::time::timeout(self.timeout, ...)` (default 5s, matches ADR-C3 §C3.7).

**`LndRpcError`** carries `Status(tonic::Code, String)`, `Transport(...)`, `Decode(#[from] prost::DecodeError)`, `Timeout(Duration)`.

**Startup-failure policy:** missing/malformed TLS cert at construction = `BuildError` (sidecar abort). LND unreachable at first poll = `ProbeResult::Failed` (matches Bitcoin pattern from ADR-C3 §C3.5). `Channel::from_shared(...).tls_config(...).connect_lazy()` is the construction shape.

### Acceptance criteria
- [ ] `LndGrpcClient::new` builds a `tonic::Channel` with `.connect_lazy()` (no network call at construction)
- [ ] TLS uses **only** the configured LND cert; native root features remain disabled
- [ ] Macaroon attached as `macaroon` metadata header on every request
- [ ] All three RPC methods present with per-request timeout
- [ ] `BuildError::MissingScheme` covers endpoints without `https://`
- [ ] `LndRpcError` includes `Decode(prost::DecodeError)` variant
- [ ] `tonic::Status → LndRpcError → CollectionErrorKind` mapping table from ADR-E2 §E2.9 applied at the collector boundary in BTH-63
- [ ] Unit tests: construction fails on bad cert / bad scheme / missing macaroon-as-metadata-value; succeeds with valid inputs

---

## BTH-63: `LndGrpcPollingCollector` — `impl PollingCollector` with parallel RPCs + `peer_online` cross-reference

**Type** Story • **Priority** High • **Estimate** M • **Component** collectors
**ADRs** E2 §E2.4 §E2.5 §E2.10; C1 §1; C3 §C3.5 §C3.6 • **Blocked by** BTH-60, BTH-62 • **Blocks** BTH-64, BTH-65

### Description

Implement `src/collectors/lnd/grpc_poll.rs` per ADR-E2 §E2.4. Mirrors `BitcoinCoreRpcCollector`: parallel RPCs via `tokio::join!`, deterministic spec-order processing, partials preserved in `ProbeResult::Failed`.

**Three RPCs per poll** (default 30s cadence):

| RPC | Produces |
|---|---|
| `GetInfo` | `StateObservation::LndNode(LndNodeState)` |
| `ListChannels` | `StateObservation::LndChannelSummary` PLUS one `StateObservation::LndChannel` per channel |
| `ListPeers` | Joined into per-channel observations as `LndChannelState.peer_online` |

**ListPeers cross-reference** (per ADR-E2 §E2.5): for each channel in `ListChannels`, look up the matching peer in `ListPeers` by `remote_pubkey`. Set `peer_online = Some(peer.online)` on success, `Some(false)` if no match, `None` only when ListPeers itself failed (so the channel observation lands as a partial).

**Health observations** per ADR-E2 §E2.10: every successful RPC emits a `HealthCheckObservation { target: "lnd.rpc.<method>", status: Ok, latency_ms }`. Every failed RPC emits the same target with `HealthStatus::Critical` and a `HealthError`.

Subject of channel observations: `EntityRef::LndChannel { node_id, channel_id }` where `channel_id` is the funding outpoint (`txid:vout` from `Channel.channel_point`).

### Acceptance criteria
- [ ] `impl PollingCollector for LndGrpcPollingCollector` with `tokio::join!` over the three RPCs
- [ ] Per-channel `LndChannelState` observations emitted with funding-outpoint-derived `LndChannelId`
- [ ] `peer_online: Some(true|false)` set when ListPeers succeeds, `None` only on ListPeers failure
- [ ] Health observations `lnd.rpc.get_info`, `lnd.rpc.list_channels`, `lnd.rpc.list_peers` emitted per call
- [ ] Partial-failure: ListPeers failure preserves channel state observations as partials with `peer_online = None`
- [ ] `BuildError::WrongTargetKind` if `CollectorTarget` isn't `LndNode`
- [ ] Construction validates shape only (no network call) per ADR-C3 §C3.5
- [ ] Tests: all-RPCs-succeed → Ok batch; ListPeers fails → Failed batch with partial LndChannel observations and `peer_online = None`; total failure → Failed batch with health observation carrying the first error

---

# Phase V0.8 — LND rules

## BTH-64: `LndChannelInactiveRule` (catalog B1) + `lnd.channel_inactive` kind binding

**Type** Story • **Priority** High • **Estimate** M • **Component** rules
**ADRs** E1 §E1.4 §E1.5; L2 §L2.1; INCIDENT_CATALOG.md §B1 • **Blocked by** BTH-61, BTH-63 • **Blocks** BTH-67

### Description

Implement the B1 rule in `src/diagnostics/rules/lnd/channel_inactive.rs`. Reads per-channel state from `StateReadModel::latest_state(EntityRef::LndChannel { node_id, channel_id }, StateName::from_well_known(LND_CHANNEL_DETAIL))`.

**Pattern:**
- **Active** when `state.active == false` for ≥ `inactive_threshold` continuous seconds. Default thresholds: **5 minutes for non-private channels (`state.private == false`)**, **30 minutes for private channels** (private channels flap more from peer NAT-traversal issues; longer window reduces false positives).
- **Cleared** when `state.active == true` is observed.
- Severity gating via `state.peer_online` (set by the BTH-63 collector cross-reference):
  - `Some(false)` (peer offline → routine cause) → severity `Warning`, confidence `Medium`
  - `Some(true)` (peer online but channel inactive → suspicious) → severity `Critical`, confidence `High`
  - `None` (peer status unavailable for this tick) → severity `Warning`, confidence `Medium` (conservative)

Fingerprint: `(EntityRef::LndChannel { node_id, channel_id }, lnd.channel_inactive, None)` — `allows_dimension = false` per BTH-61, the channel_id is already in the subject.

### Acceptance criteria
- [ ] `DiagnosticRule` impl reading `StateReadModel::latest_state` for `LndChannel`
- [ ] Active draft emitted after `inactive_threshold` continuous seconds of `active == false`
- [ ] Cleared draft emitted on first observation of `active == true`
- [ ] Different default thresholds for private vs non-private channels (5m / 30m)
- [ ] Severity / confidence gated on `peer_online` per spec above
- [ ] `kind = IncidentKind::from_well_known(LND_CHANNEL_INACTIVE)`
- [ ] Tests: short flap (<threshold) → no draft; sustained inactivity → Active; peer-online vs peer-offline severity; recovery → Cleared

---

## BTH-65: `LndChainBackendLagRule` (catalog B6) + `lnd.chain_backend_lag` kind binding

**Type** Story • **Priority** High • **Estimate** M • **Component** rules
**ADRs** E1 §E1.3 §E1.4 §E1.5; INCIDENT_CATALOG.md §B6 • **Blocked by** BTH-61, BTH-63 • **Blocks** BTH-67

### Description

Implement the B6 rule in `src/diagnostics/rules/lnd/chain_backend_lag.rs`. **Cross-source correlation** between LND's view of the chain tip and bitcoind's.

**Inputs:**

```text
lnd_height = StateReadModel::latest_state(
    EntityRef::LndNode(node_id),
    StateName::from_well_known(LND_NODE),
)?.block_height;

bitcoind_height = StateReadModel::latest_state(
    EntityRef::BitcoinNode(bitcoind_id),
    StateName::from_well_known(BITCOIN_BLOCKCHAIN),
)?.blocks;
```

**Detection:**
- **Active** when `bitcoind_height - lnd_height > lag_blocks_threshold` (default 2 blocks) for `lag_persist_seconds` (default 60 seconds) continuous. Severity `Critical`, confidence `High`.
- **Cleared** when the lag returns to `≤ 1` block.

**Correlation target:** which bitcoind to correlate against. V0.8 policy per ADR-E1 §E1.3: if the sidecar config has exactly one `BitcoinNodeId`, use it. If multiple, the rule constructor accepts a configured `chain_backend_target_bitcoind_id` (resolved from `[collectors.lnd.nodes.<id>].chain_backend_target_bitcoind_id` per ADR-X1). If not configured and multiple bitcoinds exist, the rule logs a warning and does not fire.

Fingerprint: `(EntityRef::LndNode(node_id), lnd.chain_backend_lag, None)` — `allows_dimension = false` for V0.8.

### Acceptance criteria
- [ ] `DiagnosticRule` impl reading both `LndNodeState` and `BitcoinBlockchainState`
- [ ] Default thresholds: `lag_blocks_threshold = 2`, `lag_persist_seconds = 60`
- [ ] Active draft emitted when criteria sustained; Cleared when difference ≤ 1
- [ ] Correlation target resolution: single-bitcoind auto-pick; multi-bitcoind via config; otherwise log + skip
- [ ] `kind = IncidentKind::from_well_known(LND_CHAIN_BACKEND_LAG)`
- [ ] Tests: lag-then-clear; brief blip (<persist) → no draft; multi-bitcoind unconfigured → no draft (with warning log); LND ahead of bitcoind (bitcoind lagging) → no LND-side draft

---

# Phase V0.8 — operability + e2e + docs

## BTH-66: `bithound.lnd_*` internal incident kinds + `default_kinds.toml` entries

**Type** Task • **Priority** Medium • **Estimate** S • **Component** incidents + operability
**ADRs** E2 §E2.3 (open follow-on); N1 §N1.5 (Sidecar subject pattern) • **Blocked by** BTH-61 • **Blocks** —

### Description

Register Bithound's own LND-collector incident kinds so operators can distinguish "my LND node is broken" from "Bithound's LND collector is broken." Per ADR-E2 §E2.3, the V0.8 set is:

- `bithound.lnd_unreachable` — gRPC channel connection failures
- `bithound.lnd_auth_failed` — macaroon rejected by LND
- `bithound.lnd_tls_invalid` — TLS handshake failure (likely cert mismatch)

No rules consume these in V0.8 — the registry entries land so V0.x rules can emit drafts against them via the existing health-observation surface from BTH-63 (or a future bithound-self diagnostic rule).

`well_known::ALL` additions:

```rust
pub const BITHOUND_LND_UNREACHABLE: &str = "bithound.lnd_unreachable";
pub const BITHOUND_LND_AUTH_FAILED: &str = "bithound.lnd_auth_failed";
pub const BITHOUND_LND_TLS_INVALID: &str = "bithound.lnd_tls_invalid";
```

`default_kinds.toml` entries — subject = `Sidecar`, dimension = `collector_id` (so a sidecar monitoring multiple LND nodes can fingerprint per-collector).

### Acceptance criteria
- [ ] Three new well_known constants added and in `ALL`
- [ ] `default_kinds.toml` entries land with `allowed_subjects = ["Sidecar"]`, `allows_dimension = true`, `dimension_label = "collector_id"`, `min_open_confidence = "High"`
- [ ] Bidirectional parity test (added in BTH-61) still passes
- [ ] No rule implementation required (deferred to V0.x when a self-health rule lands)

---

## BTH-67: End-to-end integration test for B1 + B6 via Polar regtest

**Type** Story • **Priority** High • **Estimate** L • **Component** tests
**ADRs** E1, E2; SPEC.md §test plan • **Blocked by** BTH-64, BTH-65 • **Blocks** BTH-68

### Description

End-to-end acceptance test that boots a regtest Bitcoin + Lightning network via [Polar](https://lightningpolar.com/) (or equivalent docker-compose) and asserts the B1 + B6 lifecycle.

**Setup:**
- 1 `bitcoind` regtest node
- 2 LND nodes (`alice`, `bob`) connected to bitcoind
- 1 channel `alice → bob`
- 1 Bithound sidecar polling both LND nodes via the BTH-63 collector

**Scenarios:**

1. **B1 channel inactive (peer-offline path):** kill `bob`'s LND container. Wait `> inactive_threshold`. Assert Bithound emits `lnd.channel_inactive` Active with `severity = Warning, confidence = Medium`. Restart bob. Assert Cleared.

2. **B1 channel inactive (peer-online path):** simulate a channel `disable_channel` while peers stay connected (LND's `lncli disablechannel`). Wait `> inactive_threshold`. Assert Active with `severity = Critical, confidence = High`. Re-enable. Assert Cleared.

3. **B6 chain-backend lag:** pause bitcoind temporarily on alice's regtest setup (`pkill -STOP bitcoind`, then `mine N blocks` on a parallel chain — or simpler: cut the gRPC connection from alice to bitcoind via iptables for the test window). Wait `> lag_persist_seconds`. Assert Bithound emits `lnd.chain_backend_lag` Active. Restore connectivity. Assert Cleared.

Polar config committed under `tests/v0_8/polar/`. Test runnable via `cargo test --test e2e_v0_8_lnd -- --ignored` (gated as `#[ignore]` because it's heavyweight; CI runs it on a manual workflow trigger or nightly).

### Acceptance criteria
- [ ] Polar config (`network.yml` or equivalent) under `tests/v0_8/polar/`
- [ ] `tests/e2e_v0_8_lnd.rs` with three test functions covering scenarios above
- [ ] Tests are `#[ignore]`-gated for the default `cargo test` run (matches existing e2e pattern from BTH-40)
- [ ] README under `tests/v0_8/` documents how to run the e2e locally
- [ ] All three scenarios pass on a clean Polar boot

---

## BTH-68: v0.0.8.0 docs refresh + CHANGELOG + VERSION bump + INCIDENT_CATALOG.md status flips

**Type** Task • **Priority** Medium • **Estimate** S • **Component** docs + release
**ADRs** — • **Blocked by** BTH-67 • **Blocks** —

### Description

Close out V0.8:

- Bump `VERSION` to `0.0.8.0`
- Add v0.0.8.0 entry to `CHANGELOG.md` summarizing the LND-first wedge (B1 + B6 implemented, LND polling collector via gRPC, ADR-E1 + E2 landed, ADR-C4 deferred to V1.0)
- Flip B1 and B6 in `docs/INCIDENT_CATALOG.md` to **"Implemented in V0.8"** under "Implemented in V0" section (which gets renamed to "Implemented")
- Add `[collectors.lnd]` shape to `docs/src/reference/config-schema.md` (mirroring `[collectors.bitcoin_core]`'s existing documentation)
- Update `docs/src/operator/install.md` with a "Configuring LND" section (TLS cert path, macaroon path, polling interval)
- Update `docs/src/operator/incident-catalog.md` to surface the two new LND kinds with operator-facing descriptions
- mdBook builds cleanly

### Acceptance criteria
- [ ] `VERSION` = `0.0.8.0`
- [ ] `CHANGELOG.md` has a v0.0.8.0 section
- [ ] `INCIDENT_CATALOG.md` B1 and B6 marked Implemented; "Implemented in V0" heading widened
- [ ] `docs/src/reference/config-schema.md` documents `[collectors.lnd]`
- [ ] `docs/src/operator/install.md` has an LND configuration section
- [ ] `docs/src/operator/incident-catalog.md` covers `lnd.channel_inactive` and `lnd.chain_backend_lag`
- [ ] `mdbook build docs/` succeeds with no warnings
