# Contributor overview

## Project shape

Bithound is **design-first and pre-runtime**. The runtime is being
assembled one ticket at a time under the BTH-N numbering scheme;
`src/main.rs` is still a placeholder.

Before doing architecture-affecting work, read these documents in
order:

1. [`SPEC.md`](https://github.com/bithoundhq/bithound/blob/main/SPEC.md) —
   the domain spec plus the accepted ADRs. Every architectural decision
   is captured here. **Code that contradicts an ADR must be rejected
   (the PR should reference the ADR first).**
2. [`IMPLEMENTATION_PLAN.md`](https://github.com/bithoundhq/bithound/blob/main/IMPLEMENTATION_PLAN.md) —
   phase structure, dependency graph, milestones, project-wide
   conventions.
3. [`TICKETS.md`](https://github.com/bithoundhq/bithound/blob/main/TICKETS.md) —
   the BTH tickets, each with the ADRs it implements, acceptance
   criteria, and `Blocked by` / `Blocks` links.
4. [`docs/INCIDENT_CATALOG.md`](https://github.com/bithoundhq/bithound/blob/main/docs/INCIDENT_CATALOG.md) —
   the diagnostic backlog.

If a ticket forces a decision the ADRs don't cover, **stop and append
a new ADR** to `SPEC.md` § 23 before coding. Don't extend the
architecture by precedent.

## Picking up work

- Filter open issues by label: `is:open label:phase:01` for the
  earliest phase, `is:open label:priority:high label:size:S` for small
  high-priority work, etc.
- Phase 1 unblocks the largest amount of downstream work; Phases 3
  (storage), 5 (engine), 6 (read models), 7 (collectors) can be
  parallelised after Phase 1+2 land.

## Where to ask questions

Open an issue or comment on the relevant ticket. Architectural
questions that don't fit a ticket belong on `SPEC.md` PRs.
