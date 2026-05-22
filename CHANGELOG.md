# Changelog

All notable changes to this project are recorded here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions are
4-digit `MAJOR.MINOR.PATCH.MICRO` (gstack convention). The project is
still pre-runtime, so we stay below `0.1.0.0` until the supervisor +
consumer wiring lands and bithound actually boots end-to-end.

## [Unreleased]

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
