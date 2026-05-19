# Bithound

Bithound is an observability sidecar for Bitcoin infrastructure. It runs
next to a Bitcoin node, observes its state, detects operational problems
via diagnostic rules, and notifies the operator over Telegram, Discord,
or a webhook.

> **Status.** Pre-runtime. The domain model exists as typed Rust; the
> runtime is being assembled one ticket at a time. These docs are
> growing alongside the implementation — most pages contain pointers to
> the in-repo source-of-truth files until the corresponding subsystem
> ships.

## What's here

- **[Operator guide](operator/install.md)** — installation, configuration,
  the built-in incident catalog, and how to add custom incidents.
- **[Contributor guide](contributor/overview.md)** — architecture sketch,
  module map, and the ticket-driven development workflow.
- **[Reference](reference/incident-kinds.md)** — schemas for the
  incident-kind catalog, observation payloads, and the sidecar config.
- **[Architecture decision records](adr/index.md)** — index of the ADRs
  that govern Bithound's design.

## Source-of-truth documents

The ADRs now live in this site under
[Architecture decision records](adr/index.md). The remaining
domain-spec and planning documents still live alongside the code:

- [`SPEC.md`](https://github.com/bithoundhq/bithound/blob/main/SPEC.md) —
  domain spec, §§ 1–22. § 23 is a pointer to the ADRs in this site.
- [`IMPLEMENTATION_PLAN.md`](https://github.com/bithoundhq/bithound/blob/main/IMPLEMENTATION_PLAN.md) —
  phase structure, dependency graph, milestones.
- [`TICKETS.md`](https://github.com/bithoundhq/bithound/blob/main/TICKETS.md) —
  the full ticket list with acceptance criteria.
- [`docs/INCIDENT_CATALOG.md`](https://github.com/bithoundhq/bithound/blob/main/docs/INCIDENT_CATALOG.md) —
  the diagnostic backlog: documented Bitcoin / LND / Elements incident
  patterns that future rules will detect.
