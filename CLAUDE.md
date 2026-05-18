# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this project is

Bithound is an observability sidecar for Bitcoin infrastructure. It runs next to a
Bitcoin node, observes its state, detects operational problems via diagnostic rules,
and notifies the operator (Telegram, Discord, webhook).

The project is **design-first and currently pre-runtime**. `src/main.rs` is still
`println!("hello world")` — the entire domain model exists as typed Rust but nothing
is wired together. Implementation is happening one ticket at a time under the BTH-N
numbering scheme. **Expect ~190 `dead_code` warnings during this phase**; they're not
bugs, they're types defined ahead of the runtime that will use them.

## Source-of-truth documents

Before doing architecture-affecting work, read in this order:

1. **`SPEC.md`** — the domain spec plus 18 accepted ADRs (001, L1–L5, R1–R3, C1–C3,
   S1–S3, P1, P2, X1). Every architectural decision is captured here. **Code that
   contradicts an ADR must be rejected** (the PR should reference the ADR first).
2. **`IMPLEMENTATION_PLAN.md`** — phase structure, dependency graph, milestones,
   estimates, project-wide conventions.
3. **`TICKETS.md`** — 41 BTH-N tickets in JIRA shape. Each ticket lists the ADRs it
   implements, acceptance criteria, and `Blocked by` / `Blocks` links.
4. **`docs/INCIDENT_CATALOG.md`** — the diagnostic backlog. ~17 documented Bitcoin /
   LND / Elements incident patterns that future rules will detect. The terminology
   here (symptom / signals / diagnosis / action / look-alikes) is intentional.

When a ticket forces a decision the ADRs don't cover, **stop and append a new ADR**
to `SPEC.md` § 23 before coding. Don't extend the architecture by precedent.

## Workflow conventions

These are conventions the user has established, not options:

- **One PR per ticket.** Branch name: `bth-<n>-<slug>` (e.g. `bth-19-engine-handle`).
  Non-ticket fixes use a descriptive slug (e.g. `fix-rust-yml-clippy-typo`).
- **PR body** includes: `Closes #<ticket-number>`, a brief Summary, the
  Acceptance Criteria checklist copy-pasted from the ticket (with done items
  checked), a Test Plan, and a "Deviations from spec" section (usually "None").
- **Commit message** format: `BTH-N: <one-line summary>` followed by a body.
  **Do not include `Co-Authored-By` or any Claude attribution** — this is an
  explicit preference.
- **Stack PRs when tickets depend on each other** (use `gh pr create --base
  <parent-branch>`). GitHub auto-rebases the base when the parent merges.
- **Never `git add -A` or `git add .`.** Stage specific files. The working tree
  often has pre-session edits to `Cargo.lock` and `src/shared/types.rs` that must
  not get pulled into a ticket commit. Stash them with `git stash push <paths>`
  if needed.
- **Don't commit unless asked.** The user merges PRs themselves on their own
  cadence; sometimes they merge between turns.
- **GitHub issue numbers do not always match BTH numbers.** BTH-1 through
  BTH-41 line up with issues #1 through #41 because the repo started with
  zero issues/PRs. From BTH-42 onward, PRs #42–#48 already consumed those
  numbers when the D-cluster issues were created, so BTH-42…BTH-50 map
  to issues #49…#57. Issue bodies and cross-references use the GitHub
  numbers (with the BTH ticket ID in parens, e.g. `Blocked by: #54 (BTH-47)`).
  Mapping table in `TICKETS.md`.
- **Stale-base PRs need 'Update branch' before merging.** When two PRs are
  merged within seconds of each other (or one PR's branch was created
  before another's recent merge), GitHub may merge the second PR with
  the *older* main commit as its first parent. The first PR's commit
  stays reachable in the DAG but its tree changes don't appear on `main`'s
  first-parent path — the PR shows MERGED, the issue stays OPEN, and
  `grep` on main can't find the changes. **Always click "Update branch"
  on the second PR before merging it.** Recovery if it happens anyway:
  cherry-pick the orphaned commit onto a fresh branch from current main
  and open a recovery PR. Bithound hit this on PR #44 (BTH-4); PR #61
  shows the recovery pattern.

## Commands

```bash
# Build
cargo check                              # fast type-check
cargo build                              # full build

# Test
cargo test                               # all tests
cargo test <module>::<test>              # single test, e.g.
cargo test incidents::types::tests::fingerprint_equality_is_structural

# Lint and format
cargo clippy -- -D warnings              # CI runs this
cargo fmt --check                        # CI runs this

# Docs
cargo doc --no-deps                      # emits target/doc/bithound/*
```

CI (`.github/workflows/rust.yml`) runs `fmt --check`, `clippy -- -D warnings`,
`build --verbose`, and `test --verbose` on every PR to `main`.

## Architecture overview

The runtime is **a single-writer pipeline**:

```
collectors → ObservationBatch → observation store
                              ↓
                         read models (apply &mut self)
                              ↓
                         diagnostic rules → IncidentSignalDraft
                                          ↓
                                    IncidentEngine.handle()  (&mut self)
                                          ↓
                                    HandleOutcome { signal_observation,
                                                    touched_incident,
                                                    lifecycle_events }
                                          ↓
                                    incident_repo.save (write-through)
                                          ↓
                                    Notifier.dispatch()  → Telegram / Discord / webhook
```

Per ADR-S1, this all runs in a **single consumer tokio task** so `&mut self` on
the read-model store and incident engine works without locks. Collectors run as
their own tasks and feed a bounded `mpsc::channel<ObservationBatch>`.

### Top-level modules

| Path | Role | ADRs |
|---|---|---|
| `src/shared/` | ID newtypes, `EntityRef`, `EntitySubjectKind`, `EvidenceRef` | L1 §3 |
| `src/observations/` | The observation envelope + 10 payload variants | R2 |
| `src/collectors/` | `PollingCollector` / `SubscriptionCollector` traits | C1–C3 |
| `src/read_models/` | Six trait surfaces + (designed) `ReadModelStore` | R1, R3 |
| `src/diagnostics/` | `DiagnosticRule` trait, `IncidentSignalDraft` | L2, R1 |
| `src/incidents/` | `Incident`, `IncidentFingerprint`, `IncidentEngine` (designed) | L1–L5 |
| `src/notifications/` | `Notifier` + Telegram/Discord/webhook target adapters | L5 |
| `src/runtime/` | (Designed, not yet present) — supervisor + consumer | S1–S3 |
| `src/storage/` | (Designed, not yet present) — sqlx-backed impls | P1, P2 |
| `src/config/` | (Designed, not yet present) — TOML + `clap` CLI | X1 |
| `migrations/` | (Designed, not yet present) — sqlx migration files | P1 |

### Key invariants that aren't obvious from the code

- **`Suppressed` in `IncidentStatus` is reserved for V0.2.** The V0/V0.1 engine
  never sets it. V0.1 suppression is **notifier-side** via `SuppressionRule`
  (per ADR-L5). The variant exists so the type is forward-compatible.
- **`StateObservation::name()` and `state/well_known.rs` must stay in sync.**
  The parity unit tests in `src/observations/types/state.rs` fail the build if
  you add a variant or constant without updating the other.
- **`IncidentFingerprint::as_key()` format is load-bearing.** It's the storage
  index key per ADR-P1; format is
  `"<subject_kind>|<subject_id>|<incident_kind>|<dimension or '-'>"`.
- **Observations are append-only facts.** Never mutate an `Observation` after
  construction; produce a new one if you need to record a change.
- **The incident engine is single-writer.** Don't add a second mutator of the
  `open_incidents` map (per ADR-L4 §L4.4 — no periodic reconciliation, no
  parallel handlers). All mutations route through `engine.handle()`.
- **Rules own their own hysteresis** (per ADR-L2 §L2.1). The engine treats every
  `Active` draft as immediate-open. Rules look back through read models to
  decide whether to emit `Active` or `Cleared`.

### Vocabulary that maps post-design ≠ pre-design

The legacy README used "probes / probe runners / reducers / snapshots". The
current vocabulary (matching SPEC.md and the existing code) is:

- collector (replaces "probe runner")
- observation (replaces "raw probe result")
- projection / read model (replaces "reducer / snapshot")
- incident engine (replaces "incident detector")
- notifier (replaces "consumer / exporter")

When writing new code, **use the new vocabulary**. If you encounter the old
vocabulary in a comment or doc, fix it as a drive-by.

## Picking up work

- Filter open Issues by label: `is:open label:phase:01` for Phase 1 work,
  `is:open label:priority:high label:size:S` for small high-priority work, etc.
- Phase 1 (BTH-1 … BTH-6) unblocks the largest amount of downstream work.
- Phases 3 (storage), 5 (engine), 6 (read models), 7 (collectors) can be done
  in parallel after Phase 1+2 land.
- BTH-19, BTH-28, BTH-33, BTH-35, BTH-40 are the L-sized stories — budget
  more iteration time on those.
