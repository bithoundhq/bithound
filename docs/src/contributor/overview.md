# Contributor overview

## Project shape

Bithound ships V0 as a single Rust binary. The runtime (collectors,
read models, diagnostic rules, engine, notifier, storage, config) is
wired end-to-end and exercised by 230+ unit tests plus one
opt-in end-to-end integration test under `tests/e2e_tip_lag.rs`.

The project is built ticket by ticket under the BTH-N numbering
scheme — one BTH ticket, one branch, one PR. See the
[ticket workflow](workflow.md) for branch naming, commit format,
and the CI gates every PR must pass.

## Before touching architecture

Read these in order — they are the source of truth, and code that
contradicts them gets rejected on review:

1. [`SPEC.md`](https://github.com/bithoundhq/bithound/blob/main/SPEC.md) —
   the domain spec, §§ 1–22. § 23 is a pointer into this site for the
   ADRs.
2. [Architecture decision records](../adr/index.md) — every accepted
   ADR, grouped by cluster (L = lifecycle, R = read models, C =
   collectors, S = supervisor/runtime, P = persistence, X = config,
   D = domain refinement, N = notifications, A = local API).
3. [`IMPLEMENTATION_PLAN.md`](https://github.com/bithoundhq/bithound/blob/main/IMPLEMENTATION_PLAN.md) —
   phase structure, dependency graph, milestones, conventions.
4. [`TICKETS.md`](https://github.com/bithoundhq/bithound/blob/main/TICKETS.md) —
   the full BTH-1 … BTH-58 ticket list, each carrying the ADRs it
   implements, acceptance criteria, and `Blocked by` / `Blocks`
   relationships.
5. [`docs/INCIDENT_CATALOG.md`](https://github.com/bithoundhq/bithound/blob/main/docs/INCIDENT_CATALOG.md) —
   the diagnostic backlog. The three rules wired in V0 cross-link
   back here.

If a ticket forces a decision the ADRs don't cover, **stop and write
a new ADR** under `docs/src/adr/` before coding. Link it from
[`docs/src/adr/index.md`](../adr/index.md) and from
`docs/src/SUMMARY.md`. Don't extend the architecture by precedent.

## What V0 covers, what V0.1 picks up

V0 covers the Phase 1–12 ticket set:

- Foundation, read-model trait surface, storage, kind registry,
  incident engine, notifications, runtime loop, the first three
  diagnostic rules, end-to-end integration test, operator docs.
- The full domain model from
  [`SPEC.md`](https://github.com/bithoundhq/bithound/blob/main/SPEC.md)
  is implemented; the runtime exercises every piece end-to-end.

V0.1 picks up:

- Subscription collectors (`BitcoinCoreZmq`, `LndGrpcStream`).
- LND and host collectors.
- Additional diagnostic rules from the catalog backlog (A4–A8, B1–B6,
  C1–C3, X1, X2).
- Suppression rules and maintenance windows.
- The retry scheduler for notification attempts (BTH-53).
- The local read-only operator HTTP API (BTH-56, BTH-57).

## Picking up work

Filter open issues by label on the
[Issues page](https://github.com/bithoundhq/bithound/issues):

- `is:open label:phase:13` and onward — the current V0.1 work.
- `is:open label:priority:high label:size:S` — small high-priority
  starter work.
- `is:open label:size:L` — larger stories that need scoping
  conversations before pickup.

Phase 1+2 are landed. Phases 3 (storage), 5 (engine), 6 (read
models), 7 (collectors) are also landed and can host parallel V0.1
work.

## Where to ask questions

Open an issue or comment on the relevant ticket. Architectural
questions that don't fit a ticket belong on a `SPEC.md` PR or a new
ADR draft under `docs/src/adr/`.
