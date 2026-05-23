# Bithound

Bithound is an observability sidecar for Bitcoin infrastructure. It
runs next to a Bitcoin node, polls it on a fixed interval, evaluates
diagnostic rules against the observed state, and notifies the
operator when a real incident lifts.

V0 is the first runnable release. It monitors one Bitcoin Core node,
ships three diagnostic rules drawn from the
[incident catalog](operator/incident-catalog.md), and routes
lifecycle events (`Opened`, `Escalated`, `Resolved`) to Telegram,
Discord, and generic webhook sinks.

## Who this book is for

- **Operators** running bithound against their own node. Start with
  [Installation](operator/install.md), then
  [Configuration](operator/configuration.md), then
  [Notifications](operator/notifications.md). The
  [Incident catalog](operator/incident-catalog.md) explains what each
  shipped rule fires on and what to do about it.
- **Contributors** writing rules or wiring new collectors. Start with
  [Contributor overview](contributor/overview.md), read the
  [Architecture](contributor/architecture.md) tour, then follow the
  per-ticket [Ticket workflow](contributor/workflow.md).
- **Anyone wiring custom incident kinds**: see
  [Custom incidents](operator/custom-incidents.md) and the
  [Incident-kind schema](reference/incident-kinds.md).

## What V0 does

- Polls one Bitcoin Core node every configured interval (typically
  10 seconds). Each poll runs four RPC calls in parallel —
  `getblockchaininfo`, `getmempoolinfo`, `getnetworkinfo`,
  `getpeerinfo` — and produces typed state observations plus per-call
  health observations.
- Evaluates three diagnostic rules against the observed state and
  lifts qualifying signals into durable incidents with fingerprint
  deduplication, monotonic severity, and a clean `Opened` /
  `Escalated` / `Resolved` lifecycle.
- Routes lifecycle events to Telegram, Discord, and generic webhook
  sinks per operator-defined rules. Every dispatch leaves an audit
  row so a delivery failure is forensic.
- Persists observations, incidents, and notification attempts to a
  local SQLite database with a configurable retention window.
- Serves an operator HTTP API on `127.0.0.1:8487` by default. Four
  read-only endpoints: `GET /health`, `GET /incidents/open`,
  `GET /incidents/:id`, `GET /incidents/:id/evidence`. Loopback bind
  is the safety mechanism; no auth, no CORS, no TLS in V0.

## What V0 doesn't do

- Monitor LND or Elements. The config schema accepts those blocks so
  V0.1 can land without migration; no collectors are wired yet.
- Subscribe to ZMQ. The `zmq_endpoint` config field is parsed but
  ignored; ZMQ collectors land in V0.1.
- Expose a browser UI. The operator API speaks JSON over loopback
  HTTP; a UI is V0.2.
- Implement suppression rules or maintenance windows. The shapes
  exist; the runtime doesn't gate on them yet.
- Auto-update or auto-recover. Use a process supervisor (`systemd`,
  Conductor, etc.); bithound itself respawns its collector tasks
  with exponential backoff per ADR-S2.

## Source-of-truth documents

These docs are the operator-facing front door. The contributor-facing
source of truth lives next to the code:

- [`SPEC.md`](https://github.com/bithoundhq/bithound/blob/main/SPEC.md) —
  the domain spec, §§ 1–22. § 23 points into this site for the ADRs.
- [Architecture decision records](adr/index.md) — every accepted ADR,
  grouped by cluster. Code that contradicts an ADR is rejected on
  review.
- [`IMPLEMENTATION_PLAN.md`](https://github.com/bithoundhq/bithound/blob/main/IMPLEMENTATION_PLAN.md) —
  phase structure, dependency graph, milestones.
- [`TICKETS.md`](https://github.com/bithoundhq/bithound/blob/main/TICKETS.md) —
  the full ticket list (BTH-1 … BTH-58) with acceptance criteria
  and the GitHub-issue mapping.
- [`docs/INCIDENT_CATALOG.md`](https://github.com/bithoundhq/bithound/blob/main/docs/INCIDENT_CATALOG.md) —
  the diagnostic backlog (~17 Bitcoin / LND / Elements incident
  patterns), three of which are wired in V0.
- [`docs/OPERATOR_GUIDE.md`](https://github.com/bithoundhq/bithound/blob/main/docs/OPERATOR_GUIDE.md) —
  the linear walkthrough that covers install → config → run →
  troubleshoot in a single document. The mdBook chapters here split
  the same material across reference-grade topic pages.
