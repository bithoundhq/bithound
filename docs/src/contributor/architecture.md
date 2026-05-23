# Architecture

> This page gives the contributor-facing tour. The authoritative
> reference is
> [`SPEC.md`](https://github.com/bithoundhq/bithound/blob/main/SPEC.md)
> plus the [ADRs](../adr/index.md); this is the index into them.

## Runtime topology

Bithound is a **single-writer pipeline** running on a tokio
multi-thread runtime. Collectors run as their own tasks and feed a
bounded `mpsc::channel<ObservationBatch>`; everything downstream of
the channel is single-consumer so `&mut self` works without locks.

```text
collectors → ObservationBatch → observation store
                              ↓
                         read models (apply &mut self)
                              ↓
                         diagnostic rules → IncidentSignalDraft
                                          ↓
                                    IncidentEngine.handle()  (&mut self)
                                          ↓
                                    Vec<IncidentEvent>
                                          ↓
                                    consumer pattern-matches each event
                                          ├── SignalRecorded   → store + read models
                                          ├── IncidentTouched  → incident_repo.save (write-through)
                                          └── Lifecycle        → notification_worker
                                                                  ↓
                                                       Telegram / Discord / webhook
```

Two task families separate the responsibilities (ADR-S1, ADR-N2):

- The **consumer task** owns the read-model store and the engine. It
  never blocks on a notification sender, so a slow webhook can't
  stall observation ingest.
- The **notification worker task** owns the senders. It reads
  dispatch messages off its own channel, flips
  `notification_attempt` rows from `Pending` to a terminal status,
  and never touches the read-model store.

Both run alongside the **supervisor task** (ADR-S2), which spawns
one worker per collector with exponential-backoff respawn (10s →
30s → 60s → 300s, resetting after a clean 5-minute run).

## Module map

| Path | Role |
| --- | --- |
| `src/main.rs` | Production bootstrap — CLI parse, config load, `runtime::run` hand-off |
| `src/shared/` | ID newtypes, `EntityRef`, `EntitySubjectKind`, `EvidenceRef` |
| `src/observations/` | Observation envelope + ten payload variants |
| `src/collectors/` | `PollingCollector` / `SubscriptionCollector` traits; `bitcoin_core_rpc` collector |
| `src/read_models/` | Six trait surfaces + the `ReadModelStore` and per-payload projections |
| `src/diagnostics/` | `DiagnosticRule` trait, `DiagnosticContext`, `IncidentSignalDraft`; the three V0 rules under `rules/bitcoin/` |
| `src/incidents/` | `Incident`, `IncidentFingerprint`, `IncidentEngine`, `KindRegistry`, well-known constants |
| `src/notifications/` | `NotificationRule`, per-sink renderers and senders (Telegram, Discord, webhook) |
| `src/runtime/` | `RuntimeDeps`, supervisor, consumer, notification worker, bootstrap helpers |
| `src/storage/` | `sqlx`-backed observation store and incident repository + memory test impls |
| `src/config/` | TOML loader + `clap` CLI + env-var override + secret resolution |
| `migrations/` | `sqlx` migration files |
| `tests/` | End-to-end integration tests (`#[ignore]`-gated) |

## Key invariants

- **The incident engine is single-writer** (ADR-L4 §L4.4). Don't add
  a second mutator of the `open_incidents` map. All mutations route
  through `engine.handle()`.
- **Rules own their own hysteresis** (ADR-L2 §L2.1). The engine
  treats every `Active` draft as immediate-open. Rules look back
  through read models, or hold their own internal state behind
  `Mutex<HashMap<EntityRef, _>>`, to debounce before emitting.
- **Observations are append-only facts.** Never mutate an
  `Observation` after construction; produce a new one if you need to
  record a change.
- **`IncidentFingerprint::as_key()` is load-bearing.** The format is
  `"<subject_kind>|<subject_id>|<incident_kind>|<dimension or '-'>"`
  per ADR-P1; it's used as the SQLite index key.
- **`Suppressed` in `IncidentStatus` is reserved for a future
  iteration.** The current engine never sets it. Suppression today
  is notifier-side via `SuppressionRule`; the runtime doesn't gate
  on it yet (V0 placeholder).
- **The engine emits `Vec<IncidentEvent>`, not a struct** (ADR-D4).
  The consumer pattern-matches `SignalRecorded`, `IncidentTouched`,
  `Lifecycle`, and `DraftBelowConfidenceFloor` and dispatches each
  side effect separately.

## Vocabulary

When writing new code, prefer the post-design vocabulary:

- *collector* (not "probe runner")
- *observation* (not "raw probe result")
- *projection* / *read model* (not "reducer" / "snapshot")
- *incident engine* (not "incident detector")
- *notifier* (not "consumer" / "exporter")

If you encounter the older vocabulary in a comment or doc, fix it as
a drive-by.

## ADR cross-reference

The ADRs back the invariants above. Quick map:

| Concern | ADR |
| --- | --- |
| Single-writer engine, locking discipline | [001](../adr/001.md), [L4](../adr/l4.md) |
| Incident fingerprint + kind registry | [L1](../adr/l1.md) |
| Signal → incident lift policy + rule hysteresis | [L2](../adr/l2.md) |
| Severity + escalation | [L3](../adr/l3.md) |
| Notifier-side suppression (V0 model) | [L5](../adr/l5.md) |
| Read-model trait surface | [R1](../adr/r1.md), [R3](../adr/r3.md) |
| Derived observations as `ObservationPayload` variants | [R2](../adr/r2.md) |
| Polling vs subscription collectors | [C1](../adr/c1.md), [C2](../adr/c2.md), [C3](../adr/c3.md) |
| Per-collector tasks + central consumer | [S1](../adr/s1.md), [S2](../adr/s2.md), [S3](../adr/s3.md) |
| SQLite via `sqlx`, storage trait shapes | [P1](../adr/p1.md), [P2](../adr/p2.md), [P3](../adr/p3.md) |
| Cross-context domain events (`Vec<IncidentEvent>`) | [D4](../adr/d4.md) |
| Validated draft + smart-constructor name newtypes | [D1](../adr/d1.md), [D2](../adr/d2.md), [D3](../adr/d3.md) |
| Identity + sub-entity scoping | [N1](../adr/n1.md) |
| Notification delivery worker (separate task) | [N2](../adr/n2.md) |
| Single TOML config, env-var overrides for secrets | [X1](../adr/x1.md) |
| Local read-only operator API (V0.1) | [A1](../adr/a1.md) |
