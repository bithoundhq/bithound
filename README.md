# Bithound

Bithound is a work-in-progress observability sidecar for Bitcoin
infrastructure. It runs next to a Bitcoin node, observes its state,
detects operational problems, and notifies the operator.

The project is in active development. The full V0 design is complete
(18 architecture decisions, see `SPEC.md`) and 41 implementation
tickets are filed in [Issues](../../issues). Code is not yet ready
for production use.

## What V0 will do

For V0, Bithound monitors a single Bitcoin Core node and detects a
small set of well-documented operational problems:

- **Bitcoin Core RPC polling** — `getblockchaininfo`,
  `getmempoolinfo`, `getnetworkinfo`, `getpeerinfo` on a configurable
  interval, producing typed state observations and per-call health
  observations.
- **Incident detection** — diagnostic rules evaluate the observed
  state against the operational catalog
  ([`docs/INCIDENT_CATALOG.md`](docs/INCIDENT_CATALOG.md)) and emit
  *incident signals* with explicit severity and confidence.
- **Incident lifecycle** — signals are lifted into durable incidents
  with fingerprint-based deduplication, monotonic severity, and
  `Opened` / `Escalated` / `Resolved` lifecycle events.
- **Notifications** — incident lifecycle events are routed to
  Telegram, Discord, or generic webhook sinks. Per-rule severity
  floors and incident-kind filters; rich delivery-receipt taxonomy
  (delivered / transient / permanent failure).
- **Local-first storage** — SQLite via `sqlx` for observations,
  incidents, and (V0.1) suppression rules. Built for a single
  binary, cloud-portable to Postgres later.

The two V0 diagnostic rules (catalog `A1` and `A3`) cover Bitcoin Core
tip lag and outbound-peer starvation. The remaining ~15 rules from the
catalog land in V0.1+.

## Architecture overview

Bithound is structured as one binary with a single-writer pipeline:

```text
collectors → observations → read models → diagnostics → incident engine → notifier
```

- **Collectors** acquire data from external sources (Bitcoin Core RPC,
  eventually ZMQ, LND, host stats) and emit `ObservationBatch`es.
- **Observations** are immutable typed facts with explicit provenance
  (subject, source collector, sidecar identity, origin).
- **Read models** are six per-observation-type projections behind a
  thin store. They serve queries to diagnostic rules.
- **Diagnostics** are stateless rules that consume read models and
  emit `IncidentSignalDraft`s. Hysteresis is rule-owned.
- **Incident engine** validates drafts against a config-driven kind
  registry, fingerprints them as `(subject, kind, dimension)`,
  manages incident lifecycle, and emits notify-worthy events.
- **Notifier** matches lifecycle events against notification rules and
  dispatches to typed sinks (Telegram, Discord, webhook).

The runtime is a tokio multi-thread setup with one task per collector
and a single consumer task that owns the read-model store and engine
state. No locking on the hot path.

## Roadmap

| Milestone | Scope |
|-----------|-------|
| V0     | Single Bitcoin Core node, two diagnostic rules, three notification sinks, SQLite storage, local config. |
| V0.1   | LND + host collectors, additional diagnostic rules, suppression rules + maintenance windows, observation-store replay. |
| V0.2   | Operator UI, acknowledge / manual resolve, dashboards, file-ref secrets. |
| V1.0+  | Cloud sync (Postgres backend), HA / multi-sidecar, plugin system. |

V0 is the current focus. See `IMPLEMENTATION_PLAN.md` for the phase
breakdown and `TICKETS.md` for the 41 implementation tickets.

## Non-goals

Bithound is not trying to be:

- a Prometheus / Grafana replacement
- a general infrastructure monitoring agent
- a wallet
- a node management daemon
- an automated recovery system

Its scope is **observation + diagnosis + notification** for
Bitcoin-adjacent infrastructure. Action and visualization remain the
operator's responsibility.

## Repository layout

```text
SPEC.md                   Domain specification + 18 ADRs (the source of truth)
IMPLEMENTATION_PLAN.md    Phases, milestones, dependency graph, estimates
TICKETS.md                JIRA-style tickets BTH-1 … BTH-41
docs/
└── INCIDENT_CATALOG.md   ~17 documented incident patterns (the diagnostic backlog)
src/                      Rust crate — currently typed domain model awaiting implementation
```

## Contributing

V0 is open for implementation work. Tickets are tagged by phase, size,
and priority — see the [Issues](../../issues) page or filter by:

- `is:open label:phase:01` — Phase 1 (foundation, unblocks everything)
- `is:open label:priority:high label:size:S` — small high-priority work
- `is:open label:size:L` — larger stories

Read `SPEC.md` and `IMPLEMENTATION_PLAN.md` before picking up a ticket.
Each issue references the ADRs it implements; deviations require a new
ADR appended to `SPEC.md` § 23 before merging.

## License

GNU GPLv3
