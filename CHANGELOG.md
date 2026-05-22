# Changelog

All notable changes to this project are recorded here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions are
4-digit `MAJOR.MINOR.PATCH.MICRO` (gstack convention).

The supervisor + consumer wiring landed in 0.0.3.0 — bithound now
boots, supervises its collectors, drives the engine + notification
worker, and shuts down cleanly on SIGINT/SIGTERM. The remaining
gap before a "real" 0.1.0.0 is the first diagnostic rules (Phase
11).

## [Unreleased]

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
