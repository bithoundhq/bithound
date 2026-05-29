# Bithound V0 Implementation Plan

Derived from `SPEC.md` ADRs 001, L1–L5, R1–R3, C1–C3, S1–S3, P1, P2, X1
(eighteen accepted decisions as of 2026-05-17). Tickets are in `TICKETS.md`.

> **Status (2026-05-29).** V0 shipped end-to-end in 0.0.5.0 (BTH-1
> through BTH-41). Phase A (local operator API, BTH-56 + BTH-57)
> shipped in 0.0.7.0. Phase D (BTH-42 through BTH-50, domain
> refinement) is interleaved with the main timeline. Phase v0.8
> (LND-first wedge, BTH-59 through BTH-66) shipped in 0.0.8.0,
> covering the typed surface + gRPC polling collector + two
> diagnostic rules — runtime wiring follows in BTH-67 (Polar e2e)
> and BTH-68. The "Deferred to V0.1+" line below predates the V0.8
> split and is now partly satisfied: most LND scope landed in V0.8;
> the host collector, suppression rules, observation-store replay,
> and retry scheduler stay in V0.9+.

## Goals

- Reach a runnable V0 sidecar that monitors a single Bitcoin Core node,
  detects two or three diagnostic conditions, and delivers notifications
  over Telegram, Discord, or a webhook.
- Maintain every architectural invariant the ADRs settled
  (single-writer pipeline, separation of engine/notifier/storage,
  type-safe domain model, config-driven kind registry, etc.).
- Keep PRs reviewable: 41 tickets averaging ½–3 days each, with
  explicit dependencies and ten integration milestones.

## Scope boundaries

**In scope:** everything the V0 column of the ADR capability tables
covers — engine + lift policy + severity + fingerprinting; six
read-model projections; `BitcoinCoreRpcCollector`; the three notifier
senders implemented for real; SQLite storage; TOML config; runtime loop;
two diagnostic rules from the catalog.

**Deferred to V0.1+:** subscription collectors (`BitcoinCoreZmq`,
`LndGrpcStream`); LND/Elements collectors; host collector;
suppression rules + maintenance windows; operator UI; cloud sync;
HA / multi-sidecar. Each has a designed surface to land against later.

## Phase summary

| # | Phase                              | Tickets   | Spec refs                |
|---|------------------------------------|-----------|--------------------------|
| 1 | Foundation & cleanups              | BTH-1–6, **BTH-54** | ADR-001, L1, R2, **N1** |
| 2 | Read-model trait surface           | BTH-7–8   | ADR-R1, 001              |
| 3 | Storage layer (SQLite)             | BTH-9–14, BTH-51, BTH-52 | ADR-P1, P2, **P3** (audit-only V0), L4, L5 |
| 4 | Kind registry                      | BTH-15–16 | ADR-L1                   |
| 5 | Incident engine                    | BTH-17–19 | ADR-L1–L4                |
| 6 | Read-model store                   | BTH-20–25 | ADR-R1, R3               |
| 7 | Collector layer                    | BTH-26–28 | ADR-C1, C2, C3           |
| 8 | Notifier sender (V0: webhook only) | **BTH-29** | (no new ADR — code work) |
| 9 | Config loading                     | BTH-32–33 | ADR-X1                   |
| 10| Runtime loop                       | BTH-34–37, **BTH-55** | ADR-S1, S2, S3, **N2** |
| 11| First diagnostic rules             | BTH-38, BTH-39, **BTH-58** | ADR-L2, R1; review §3 V0 rules |
| 12| End-to-end verification & docs     | BTH-40–41 | —                        |
| A | Local operator API                 | **BTH-56, BTH-57** | **ADR-A1**           |
| D | Domain refinement (DMMF alignment) | BTH-42–50 | ADR-D1, D2, D3, D4       |
| V0.1 | (deferred from V0; mostly absorbed by Phase v0.8) | BTH-30, BTH-31, BTH-53 | ADR-P3 §§P3.5–P3.8 |
| v0.8 | LND-first wedge                 | **BTH-59–69** | **ADR-E1, E2**       |

**Phase D** can run in parallel with phases 3–10 once Phase 1 is done.
BTH-47 (Unvalidated/Validated split) and BTH-48 (ActorId + commands)
are prerequisites for the **re-scoped** BTH-17 / BTH-19 / BTH-35. The
name-newtype migration (BTH-42 → BTH-43 → BTH-44/45 → BTH-46) can
proceed independently and incrementally.

**Phase A** (local operator API) can land any time after Phase 3 (it
reads from the repositories). It does not block end-to-end testing
but closes the V0 product loop — without it, operators can only see
incidents via notifications.

**V0 product thesis (per architecture review):** a sidecar that
monitors one Bitcoin Core node, detects three operational incidents
(`bitcoin.rpc_unreachable`, `bitcoin.no_peers`, `bitcoin.tip_lag_or_ibd_stalled`),
persists evidence, emits lifecycle events, notifies via webhook, and
exposes open incidents through a local API. Telegram, Discord, retry
queue, and suppression all move to V0.1.

**ADR-P3 additions** to Phase 3 / Phase 10 (notification attempts +
durable retry). BTH-51 + BTH-52 land alongside the existing storage
tickets; BTH-53 lands alongside BTH-35 (consumer) and the
`Notifier::dispatch` signature change rolls into BTH-29/30/31.

## Milestones

Each milestone is a meaningful integration point — code compiles,
tests run, and the system can be demoed at that stage.

| ID | Milestone                                                                | Closes after |
|----|--------------------------------------------------------------------------|--------------|
| M1 | Foundation compiles. Types ready for downstream work.                    | BTH-8        |
| M2 | Storage layer end-to-end with in-memory tests.                           | BTH-14       |
| M3 | Engine accepts drafts, persists incidents, emits lifecycle events.       | BTH-19       |
| M4 | Read-model store populated from observations; diagnostics can query.     | BTH-25       |
| M5 | `BitcoinCoreRpcCollector` produces real `ObservationBatch`es.            | BTH-28       |
| M6 | Notifications actually leave the process (webhook, Telegram, Discord).   | BTH-31       |
| M7 | Sidecar runs from `bithound.toml`; lifecycle threading verified.         | BTH-37       |
| M8 | First rules fire and produce real incidents end-to-end.                  | BTH-39       |
| M9 | V0 ship-ready with integration tests and operator docs.                  | BTH-41       |
| MD | Domain model fully DMMF-aligned (smart constructors, validated drafts, events-only output, full command vocabulary). | BTH-50 |

## Dependency graph

Critical path (longest single-thread dependency chain):

```
BTH-1 → BTH-2 → BTH-3 → BTH-4 → BTH-17 → BTH-18 → BTH-19 ─┐
                                                          │
BTH-1 → BTH-9 → BTH-11 ──────────────────────────────────┐│
        BTH-9 → BTH-12 ──────────────────────────────────┘│
                                                          │
                                                          ▼
                                          BTH-37 → BTH-38/39 → BTH-40/41
                                                          ▲
                                                          │
BTH-7 → BTH-20 → BTH-21–24 → BTH-25 ──────────────────────┤
                                                          │
BTH-26 → BTH-27 → BTH-28 ─────────────────────────────────┤
                                                          │
BTH-29 / BTH-30 / BTH-31 (parallel) ──────────────────────┤
                                                          │
BTH-32 → BTH-33 ──────────────────────────────────────────┘
```

Many sub-chains can be developed in parallel after BTH-1, BTH-2, and
the type cleanups (Phase 1) land.

### Parallelization friendly subsets

After Phase 1 + Phase 2 (BTH-1 through BTH-8), four independent work
streams open up:

- **Engine stream:** BTH-15 → BTH-16 → BTH-17 → BTH-18 → BTH-19
- **Storage stream:** BTH-9 → BTH-10 → BTH-11 → BTH-12 → BTH-13 → BTH-14
- **Read-model stream:** BTH-20 → BTH-21–24 → BTH-25
- **Collector + RPC stream:** BTH-26 → BTH-27 → BTH-28

They merge at the runtime layer (Phase 10).

The notifier senders (Phase 8) are fully independent and can land any time
after BTH-1.

## Estimates

| Size | Effort       | Tickets |
|------|--------------|---------|
| S    | ½–1 day      | 23      |
| M    | 1–2 days     | 23      |
| L    | 3–5 days     | 7       |

**Total:** ~65–100 person-days for a single contributor working end to
end (Phases 1–12 + Phase D + ADR-P3 additions combined). A small team
of 3 with the dependency graph above can ship V0 in ~5 calendar weeks.

## Quality gates per ticket

Every ticket lists explicit acceptance criteria, but each PR also must
satisfy the project-wide gates:

- `cargo check` and `cargo build` pass with no warnings introduced.
- `cargo test` passes (including new tests added for the ticket's
  acceptance criteria).
- `cargo clippy -- -D warnings` passes.
- `cargo fmt --check` passes.
- Public items are documented (`/// …`) where they're exposed beyond
  their module.

CI configuration is not in the V0 ticket set; PR #1 (BTH-1) bundles a
minimal `.github/workflows/ci.yml` that runs the four checks above.

## Stretch items (not part of V0)

Tracked in `SPEC.md` § 20.6:

- **V0.1:** subscription collectors, replay-on-startup for read models,
  retention/rotation tuning, suppression rules + maintenance windows,
  LND/Elements/Host collectors.
- **V0.2:** operator UI, Acknowledge/manual-Resolve commands,
  observation-store dashboards, file-ref secrets.
- **V1.0:** cloud sync (Postgres backend swap), HA/multi-sidecar,
  plugin system for collectors and rules.

## Conventions

- **Branch naming:** `bth-<ticket-number>-<slug>`, e.g.
  `bth-19-engine-handle`.
- **Commit message:** include `BTH-N` in the subject for tracking.
- **PR description:** state ticket ID, acceptance-criteria checklist
  copy-pasted from the ticket, and any deviations from the spec.
- **PR review:** one reviewer minimum. ADR-referenced behavior changes
  flagged inline.
- **Tests:** every public function gains at least one unit test by the
  end of its ticket. Integration tests live in `tests/`.

## When to amend the spec

If a ticket forces a decision the ADRs don't already cover, the rule is:

1. Pause the ticket.
2. Open a follow-up ADR in `SPEC.md` § 23.
3. Get the ADR approved.
4. Resume the ticket, referencing the new ADR.

Don't merge implementation that contradicts an ADR without going
through this loop.
