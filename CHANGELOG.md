# Changelog

All notable changes to this project are recorded here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions are
4-digit `MAJOR.MINOR.PATCH.MICRO` (gstack convention).

The supervisor + consumer wiring landed in 0.0.3.0 — bithound now
boots, supervises its collectors, drives the engine + notification
worker, and shuts down cleanly on SIGINT/SIGTERM. The first three V0
diagnostic rules (Phase 11) ship in 0.0.4.0. 0.0.5.0 closes Phase 12:
an end-to-end smoke test that drives the full pipeline through a
mock bitcoind, plus a refreshed README and a new operator guide.

## [Unreleased]

## [0.0.7.0] - 2026-05-23

### Added

Phase A — local operator HTTP API. Closes the V0 operator loop:
without it, "operator knows what's wrong" depends entirely on push
notifications. Two tickets, both shipped:

- **BTH-56: axum HTTP server bootstrap.** New `[api]` config block
  (defaults: `bind = "127.0.0.1:8487"`, `enabled = true`) and a third
  tokio task spawned by `runtime::run` alongside the consumer and
  notification worker. Graceful shutdown rides the existing broadcast
  channel. Loopback default is the safety mechanism — V0 has no auth,
  no CORS, no TLS.
- **BTH-57: V0 operator endpoints.** Four read-only routes:

  - `GET /health` — sidecar liveness + DB connectivity probe.
  - `GET /incidents/open` — incidents with `status != Resolved`,
    newest first.
  - `GET /incidents/:id` — full detail, 404 on unknown id, 400 on
    non-UUID input.
  - `GET /incidents/:id/evidence` — dereferenced evidence
    observations. Rows swept by retention are silently omitted from
    the response.

  Wire DTOs live in `src/api/dto.rs` separate from the domain types
  so the wire format can evolve cautiously. `SubjectDto` renders both
  `node_id` and the sub-id for scoped `EntityRef` variants so
  operators can distinguish `lnd-a/chan-1` from `lnd-b/chan-1`.

Both `IncidentRepository` and `ObservationStore` gain a `get_by_id`
method so the API can answer single-row reads through the primary-key
index instead of iterating; SQLite and in-memory impls both
implemented.

### Fixed

Pre-landing review found four small issues; all fixed in the same
version:

- `/health.uptime_seconds` reports time since sidecar start, not time
  since the API task started. Was off by however long the consumer +
  notification worker took to spin up; operators monitoring "is the
  sidecar fresh after a restart?" now get the right number.
- API 500 responses and the corresponding `tracing::error!` log now
  include the underlying storage error message. The `ApiError`
  display strings previously dropped the `RepoError` / `StoreError`
  detail, leaving operators staring at "storage error: storage error"
  in their logs.
- `/incidents/:id/evidence` `origin` field uses a stable enum→string
  mapping on `ObservationOrigin` instead of `format!("{:?}", ...)`.
  A future variant rename can't silently change the API.
- `tower` moved to `[dev-dependencies]` — it was a build-time only
  helper for the integration tests.

304 tests pass (was 286 before Phase A).

## [0.0.6.1] - 2026-05-23

### Changed

- **BTH-52: `SqliteNotificationAttemptRepository`.** Amends
  `migrations/0001_initial.sql` with the `notification_attempts`
  table (per ADR-P3 §P3.2 — every column, all four indexes,
  `STRICT`) and adds the SQLite-backed repo at
  `src/storage/sqlite/notification_attempt_repository.rs`. The
  runtime now persists every notification dispatch attempt: each row
  carries the target kind, redacted target summary, lifecycle kind,
  attempt number, and the full `DeliveryOutcome` JSON for audit.
  Secrets are never written — `target_summary` is the only target
  column. Test coverage round-trips every `DeliveryOutcome` variant
  (including `ExternalMessageRef::{Telegram, Discord}`) and verifies
  via `EXPLAIN QUERY PLAN` that `list_retryable` hits
  `idx_attempts_status_next_retry` rather than scanning. `main.rs`
  switches the runtime wiring from the in-memory repo to the SQLite
  repo so audit rows now survive a restart.
- **BTH-54 retroactive bump.** BTH-54 (Identity refinements —
  `EntityRef::Sidecar` + sub-entity scoping) merged before this
  bump; this VERSION change captures both BTH-52 and BTH-54.

## [0.0.6.0] - 2026-05-23

### Changed

Phase D — DMMF domain refinement. Seven tickets aligning Bithound's
type system with Wlaschin's "Domain Modeling Made Functional"
patterns (ADRs D1–D4; BTH-47 and BTH-48 landed earlier).

- **BTH-42: `parse_dotted_name` scaffolding.** New
  `src/shared/parse.rs` defines the shared dotted-namespace grammar
  (`[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+`, 1–128 chars) and the
  positional `ParseDottedNameError` returned by every smart-constructor
  parse failure.
- **BTH-43..BTH-46: name-newtype migration.** All ten name newtypes
  (`IncidentKind`, `MetricName`, `SignalName`, `StateName`,
  `HealthTargetId`, `CapabilityName`, `EventName`, `TransitionName`,
  `InventoryName`, `DiagnosisName`) now have private inner fields and
  are constructed only through `parse` or `from_well_known`. Serde
  `try_from = "String"` re-validates malformed names at deserialization
  time. The catalog TOML parser deserializes incident-kind names
  through the smart constructor, so a malformed name in
  `default_kinds.toml` (or a user override) is surfaced as
  `RegistryError::InvalidToml`. The `Observation` constructors that
  used to take `impl Into<String>` (`metric`, `capability`, `event`,
  `health`, `inventory`, `transition`) now take typed names; callers
  validate before constructing.
- **BTH-49: suppression vocabulary.** New
  `src/incidents/suppression.rs` defines `SuppressionCommand` and the
  `SuppressionService` async trait per ADR-D3. V0/V0.1 ship no
  concrete impl; the types stabilize the shape for V0.2 wiring.
- **BTH-50: per-context events + `DomainEvent` envelope.** Each
  domain context (observations, read_models, diagnostics, incidents,
  notifications) now owns an `events.rs` module per ADR-D4. The new
  top-level `crate::domain_events::DomainEvent` enum sums them with
  `From<*Event>` impls. V0 does not run an event bus; the enums are
  type-level documentation today and cloud-sync-ready tomorrow.

No behavioral changes for operators: the pipeline still observes,
detects, and notifies exactly as in 0.0.5.x. The migration is
type-system-only and is mechanical from the caller's side
(`IncidentKind("foo".into())` → `IncidentKind::parse("foo")?`).

## [0.0.5.1] - 2026-05-22

### Changed
- mdBook documentation refreshed against the shipped V0 surface
  (11 pages, no source-file changes). The landing page, every
  operator chapter, every contributor chapter, and every reference
  chapter now reflect what V0 actually does instead of carrying
  pre-runtime "stub" placeholders.
  - **Operator pages.** `install.md` covers `cargo install`,
    filesystem layout, a systemd unit sketch, and `--check-config`
    verification. `configuration.md` covers the minimal config,
    inline-secret guard, env-override syntax, and section-by-section
    reference. `notifications.md` covers per-sink config shapes,
    the actual JSON POST body that ships, per-rule filters, and the
    `notification_attempts` audit table. `incident-catalog.md`
    replaces the placeholder kind list with the three shipped V0
    rules and links each one to its source file.
  - **Contributor pages.** `overview.md` covers the V0 vs V0.1 scope
    boundary and the source-of-truth reading order. `architecture.md`
    updates the pipeline diagram to the `Vec<IncidentEvent>` shape
    (ADR-D4), splits the consumer + notification-worker task
    families, and includes the now-shipped `src/runtime/`,
    `src/storage/`, `src/config/` modules in the module map. The
    ADR cross-reference table maps each invariant to its ADR.
    `workflow.md` documents the phase-bundle convention and the
    version-prefixed PR-title rule.
  - **Reference pages.** `config-schema.md` replaces the
    speculative sketch with the actual TOML schema (every section,
    required/optional/default markers, inline-secret rules, env
    override). `observation-payloads.md` documents all ten payload
    variants with V0 use-frequency, plus the eight state
    sub-variants with their fields. `incident-kinds.md` refreshes
    its example to a real V0 kind.

The mdBook build (`mdbook build`) writes 42 HTML pages cleanly; no
internal cross-link is broken.

## [0.0.5.0] - 2026-05-22

### Added
- End-to-end integration test under `tests/e2e_tip_lag.rs` that
  spawns the `bithound` binary as a child process, points it at a
  hand-rolled JSON-RPC mock server returning the A1 firing pattern,
  drives a mock webhook receiver, and asserts an `Opened
  bitcoin.tip_lag_or_ibd_stalled` lifecycle event arrives within 30
  seconds. `#[ignore]`-gated so `cargo test` in CI doesn't spin a
  bithound subprocess on every run; opt in with `cargo test
  --ignored --test e2e_tip_lag` or `BITHOUND_TEST_REGTEST=1 cargo
  test --test e2e_tip_lag -- --ignored`.
- `tests/README.md` documents the integration-test layout, how to
  opt in, and the contract for adding new `tests/e2e_*.rs` scenarios.

### Changed
- `README.md` rewritten for V0: real Quick Start section that works
  for a fresh user, the three diagnostic rules listed explicitly,
  what V0 doesn't do called out separately, repo layout updated to
  cover `tests/` and the operator guide.
- `docs/INCIDENT_CATALOG.md` cross-references its V0-implemented
  entries (A1, A2, A3) to the rule modules under
  `src/diagnostics/rules/bitcoin/`, and adds a new "Implemented in
  V0" overview at the top of the file. `bitcoin.rpc_unreachable` is
  documented inline as an operability rule that doesn't map to a
  catalog entry.

### Added (docs)
- New `docs/OPERATOR_GUIDE.md` covering install, `bithound.toml`
  authoring, Bitcoin Core RPC setup (both `user_pass` and
  `cookie_file` auth schemes), Telegram / Discord / generic-webhook
  sinks with sample payloads, per-rule reference for the three V0
  diagnostic rules with "what to do" sections, log + database file
  locations, and a troubleshooting section keyed on `EX_CONFIG=78`
  and the `notification_attempts` audit table.

## [0.0.4.0] - 2026-05-22

### Added
- First V0 diagnostic rules (Phase 11) in `src/diagnostics/rules/bitcoin/`:
  - `BitcoinRpcUnreachableRule` (`bitcoin.rpc_unreachable`) — Active
    when all four Bitcoin RPC health targets (`getblockchaininfo`,
    `getmempoolinfo`, `getnetworkinfo`, `getpeerinfo`) report
    `HealthStatus::Critical` for ≥ 60 seconds; Cleared on any
    target's return to `Ok`. Confidence High, severity Critical.
  - `BitcoinNoPeersRule` (`bitcoin.no_peers`) — Active when
    `connections_out == 0` AND `networkactive == true` for ≥ 60
    seconds; Cleared when an outbound peer reappears. Stays silent
    when the operator has disabled networking. Confidence High,
    severity Critical. Tightens catalog entry A3 (<8 peers) to the
    unambiguous zero case.
  - `BitcoinTipLagOrIbdStalledRule` (`bitcoin.tip_lag_or_ibd_stalled`)
    — Active when either A1 (IBD true, `headers - blocks < 1000`,
    `verification_progress > 0.999`, peer_count ≥ 8) or A2
    (`headers - blocks ≥ 1000` AND `verification_progress` flat
    over a 5-minute window) holds across two consecutive ticks;
    Cleared when neither holds for two consecutive ticks.
    Confidence High, severity Critical.
- `IncidentKind::from_well_known(&'static str) -> IncidentKind`
  helper so rules can reference kinds via the constants in
  `src/incidents/well_known.rs` rather than re-typing string literals.
- `SignalName::for_incident_kind(&IncidentKind)` helper centralizes the
  rule-to-signal-name mapping (`"{kind}.signal"`) so the suffix can't
  drift across the codebase. Existing test fixtures in
  `runtime/consumer.rs`, `storage/sqlite/observation_store.rs`, and
  `observations/types.rs` updated to use it.
- `DiagnosticContext` gains a `monotonic_now: std::time::Instant`
  field alongside `now: DateTime<Utc>`. Rules use the monotonic clock
  for all debounce timing, so a backwards wall-clock jump (NTP
  correction, VM suspend/resume) cannot stall an open incident.
- `config/default_kinds.toml` now ships entries for the three V0 kinds
  (`bitcoin.rpc_unreachable`, `bitcoin.no_peers`,
  `bitcoin.tip_lag_or_ibd_stalled`) with `min_open_confidence = "High"`
  and `allowed_subjects = ["BitcoinNode"]`. A parity test in
  `well_known` fails the build on drift between the constants and the
  embedded catalog.
- `StateReadModelExt` is now re-exported from `crate::read_models` so
  rule code can call `ctx.state.bitcoin_blockchain(...)` etc. without
  reaching into the trait submodule.
- `BitcoinCoreRpcCollector` exports `HEALTH_TARGETS` (the canonical
  ordered list of its four RPC health-check target names). Rules
  reference this slice so a renamed target in the collector can't
  silently drift from its consumers.
- `main.rs` wires all three rules into `RuntimeDeps::rules` at boot.

### Hardening (review-driven)
- All three rules use poison-safe `Mutex` recovery: a panic inside one
  `evaluate` call no longer cascades through the consumer task. The
  next tick rebuilds the counters from observed state.
- Per-subject state maps prune idle entries after one hour with no
  open incident, so long-running sidecars whose subject set churns
  don't grow the map without bound.
- `debug_assert!(debounce_ticks >= 1)` in
  `BitcoinTipLagOrIbdStalledRule::with_settings` rejects the zero-tick
  configuration that would collapse the rule to fire-on-first-tick.

## [0.0.3.0] - 2026-05-22

### Added
- Runtime layer (Phase 10): `src/runtime/` with `supervisor`,
  `consumer`, `notification_worker`, `bootstrap`, and `mod.rs` —
  the end-to-end pipeline that ADRs S1, S2, S3, and N2 describe.
- `supervisor::run` spawns one tokio task per polling collector
  and one per subscription collector. Each task respawns its work
  loop on panic / Err with exponential backoff (10s/30s/60s/300s,
  reset after a clean 5-minute run) and exits cleanly on the
  shared broadcast shutdown signal.
- `consumer::run` is the central single-writer task: per batch it
  appends to the observation store, applies to the read models,
  derives a `DiagnosticContext` and evaluates every rule under
  `std::panic::catch_unwind` (a panic in one rule never poisons
  the cycle), then pattern-matches the engine's `Vec<IncidentEvent>`
  to drive persistence + notification handoff. Per ADR-D4 the
  consumer reads the event vector; per ADR-N2 it never calls a
  sender directly — lifecycle events INSERT Pending attempt rows
  and surface a `NotificationDispatch` to the worker.
- `notification_worker::run` consumes the dispatch channel, fans
  out to the right sender per target (Webhook / Telegram /
  Discord), and flips each row to its terminal status. The
  two-writer invariant holds: if the worker dies mid-dispatch,
  the row stays `Pending` forever as audit trail.
- `bootstrap` helpers build the runtime collaborators from the
  parsed config: `node_registry_from_config`,
  `build_polling_collectors`, `build_subscription_collectors`.
  V0 implements the `BitcoinCoreRpc` integration; other kinds
  return `BuildError::NotImplemented` with a stable kind name.
- `runtime::run` (top-level) wires the bounded mpsc channels,
  spawns all three task families into a `JoinSet`, awaits
  SIGINT/SIGTERM, broadcasts shutdown, and joins with a deadline
  (`RuntimeConfig::shutdown_deadline_seconds`, default 30s);
  remaining tasks are aborted if the deadline expires.
- `src/main.rs` is now the production bootstrap: `tracing_subscriber`
  + EnvFilter init, CLI parse, `--version` / `--check-config`,
  `Config::load_from_args_and_env`, full `RuntimeDeps`
  construction (including secret resolution into `NotificationRule`
  targets), and the `runtime::run` hand-off. Exit code 78
  (`EX_CONFIG`) on any startup failure.

### Changed
- `DiagnosticRule` now requires `Send + Sync` — the runtime stores
  rules behind `Box<dyn DiagnosticRule>` and the consumer task
  moves the whole vec across an await boundary.
- `tokio = { features = ["full", "test-util"] }` is now a
  dev-dependency so the supervisor / consumer / worker tests can
  drive paused time deterministically.

## [0.0.2.0] - 2026-05-22

### Added
- Storage layer (Phase 3): `ObservationStore` and `IncidentRepository`
  trait surfaces in `src/storage/traits.rs`, plus the
  `NotificationAttemptRepository` trait in `src/notifications/repository.rs`.
- SQLite-backed impls under `src/storage/sqlite/`:
  `observation_store.rs` (append/stream observations, WAL-friendly),
  `incident_repository.rs` (open-incident persistence, load_open at
  boot).
- Retention background task in `src/storage/retention.rs` —
  enforces the configured `[storage.retention]` windows on a periodic
  timer, exits cleanly on shutdown.
- In-memory test impls of every repository trait under
  `src/storage/memory/`. These are the impls the Phase 10 runtime
  integration tests will use; the SQLite impls take over in
  production.
- Revised `NotificationAttempt` carrying `lifecycle_kind`,
  `target_kind` + `target_summary` (never the SecretString itself),
  `attempt_number`, retry chaining (`parent_attempt_id`,
  `next_retry_at`), and outcome/external_ref columns — the audit
  shape the V0 worker will produce.

### Changed
- `IncidentRepository` interface expanded with the methods the
  consumer + bootstrap need (`save`, `load_open`).

## [0.0.1.0] - 2026-05-22

### Added
- Config layer (`src/config/`): TOML-shape `serde::Deserialize` types
  for every V0 section, a clap-derived CLI exposing `--config`,
  `--check-config`, and `--version`, and `examples/bithound.example.toml`
  as a copyable sample.
- `Config::load_from_args_and_env` running the full eight-step
  bootstrap — config-path resolution, `BITHOUND_*` env overrides,
  inline-secret rejection, cross-reference validation, env-var
  presence checks, `SecretString` resolution, sidecar-ID
  persistence, and SQLite pool open. Returns a `LoadedConfig` bundle
  the runtime layer will consume.
- Inline-secret guard rejects any field named `*_password`,
  `*_token`, or `*_secret` (or the bare words) that lacks the
  mandatory `_env` suffix, with a dotted path to the offending key.

### Changed
- `src/main.rs` now parses the CLI, dispatches to the loader, and
  exits 78 (`EX_CONFIG`) on any load failure. The post-load
  runtime hand-off is still a one-line summary print pending the
  supervisor + consumer wiring.

## [0.0.0.2] - 2026-05-22

### Added
- `CHANGELOG.md` so future PRs can record changes here as part of the
  standard release flow.

## [0.0.0.1] - 2026-05-22

### Added
- `VERSION` file so tooling has a stable source of truth for the
  current release number and PR titles can be version-prefixed.
