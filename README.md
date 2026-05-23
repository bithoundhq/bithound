# Bithound

Bithound is an observability sidecar for Bitcoin infrastructure. It
runs next to a Bitcoin node, polls it on a fixed interval, evaluates
diagnostic rules against the observed state, and notifies the operator
when a real incident lifts.

V0 is the first runnable release. It monitors one Bitcoin Core node,
ships three diagnostic rules from the
[incident catalog](docs/INCIDENT_CATALOG.md), and routes lifecycle
events (`Opened` / `Escalated` / `Resolved`) to Telegram, Discord,
and generic webhook sinks.

For a full walk through install → config → run → troubleshoot, see
the [Operator Guide](docs/OPERATOR_GUIDE.md).

## Quick start

```bash
# Build from source.
git clone https://github.com/bithoundhq/bithound.git
cd bithound
cargo install --path . --locked
```

Create `/etc/bithound/bithound.toml`:

```toml
[sidecar]
id_file = "/var/lib/bithound/sidecar_id"
log_level = "info"

[storage]
db_path = "/var/lib/bithound/bithound.db"

[[bitcoin_nodes]]
id = "alice"
rpc_url = "http://127.0.0.1:8332"

[bitcoin_nodes.auth]
type = "cookie_file"
path = "/var/lib/bitcoind/.cookie"

[[collectors]]
id = "alice-rpc"
target = { type = "bitcoin_node", id = "alice" }
integration = { type = "bitcoin_core_rpc", interval_seconds = 10 }
instance_label = "alice"

[[notification_rules]]
id = "critical-to-webhook"
name = "Critical incidents → webhook"
enabled = true
min_severity = "critical"
event_kinds = []

[notification_rules.target]
type = "webhook"
url_env = "BITHOUND_OPS_WEBHOOK"
```

Run it:

```bash
sudo mkdir -p /var/lib/bithound
sudo chown bithound:bithound /var/lib/bithound

BITHOUND_OPS_WEBHOOK="https://hooks.your-incident-bus.example/incoming" \
  bithound --config /etc/bithound/bithound.toml
```

You should see one `bithound runtime starting` line on stderr,
followed by per-collector load lines. If you see an `EX_CONFIG=78`
exit, read the error message — it names the offending key.

The [full example config](examples/bithound.example.toml) covers
every supported field. The [Operator Guide](docs/OPERATOR_GUIDE.md)
walks each section in plain English, including Bitcoin Core RPC
setup and notification-sink credentials.

## What V0 does

- **Bitcoin Core RPC polling** — four calls in parallel
  (`getblockchaininfo`, `getmempoolinfo`, `getnetworkinfo`,
  `getpeerinfo`) on the configured interval, producing typed state
  observations and per-call health observations.
- **Incident detection** — three diagnostic rules from the
  [catalog](docs/INCIDENT_CATALOG.md): `bitcoin.rpc_unreachable`,
  `bitcoin.no_peers`, `bitcoin.tip_lag_or_ibd_stalled`.
- **Incident lifecycle** — signals are lifted into durable
  incidents with fingerprint-based deduplication, monotonic
  severity, and `Opened` / `Escalated` / `Resolved` lifecycle
  events.
- **Notifications** — lifecycle events route to Telegram, Discord,
  or generic webhook sinks with per-rule severity floors and
  kind filters. Every attempt persists to an audit table so a
  delivery failure leaves a forensic trail.
- **Local-first storage** — SQLite via `sqlx` for observations,
  incidents, and notification attempts. Built for a single binary,
  cloud-portable to Postgres later.

## What V0 doesn't do

- Monitor LND or Elements (config schema accepts those blocks so
  V0.1 can land without migration; no collectors are wired yet).
- Subscribe to ZMQ (the `zmq_endpoint` field is parsed but ignored).
- Expose a UI (read-only HTTP API lands in the **A** cluster
  tickets — BTH-56, BTH-57).
- Implement suppression rules or maintenance windows (designed,
  not yet wired).
- Auto-update or auto-restart on crash (use systemd / a process
  supervisor of your choice; the binary itself respawns its
  collector tasks with exponential backoff per ADR-S2).

## Architecture

```text
collectors → observations → read models → diagnostics → incident engine → notifier
```

- **Collectors** acquire data from external sources and emit
  `ObservationBatch`es. V0 ships one: the Bitcoin Core JSON-RPC
  collector under `src/collectors/bitcoin_core/rpc.rs`.
- **Observations** are immutable typed facts with explicit
  provenance (subject, source collector, sidecar identity, origin).
- **Read models** are six per-observation-type projections behind
  a thin store. They serve queries to diagnostic rules without
  letting rules touch raw observation history.
- **Diagnostics** are rules that consume read models and emit
  `IncidentSignalDraft`s. Hysteresis is rule-owned (per ADR-L2).
- **Incident engine** validates drafts against a config-driven
  kind registry, fingerprints them as
  `(subject, kind, dimension)`, manages incident lifecycle, and
  emits notify-worthy events.
- **Notifier** matches lifecycle events against notification rules
  and dispatches to typed sinks. The dispatch worker runs as a
  separate task so a slow webhook can't block the consumer
  (per ADR-N2).

The runtime is a tokio multi-thread setup. The consumer task is a
single writer that owns the read-model store and engine state, so
no locking on the hot path. Collectors run as their own tasks with
exponential-backoff respawn.

## Roadmap

| Milestone | Scope |
|-----------|-------|
| V0    | Single Bitcoin Core node, three diagnostic rules, three notification sinks, SQLite storage, local config. |
| V0.1  | LND + host collectors, additional diagnostic rules, suppression rules + maintenance windows, observation-store replay, retry scheduler. |
| V0.2  | Operator UI, acknowledge / manual resolve, dashboards, file-ref secrets. |
| V1.0+ | Cloud sync (Postgres backend), HA / multi-sidecar, plugin system. |

V0 ships in phases tracked in `IMPLEMENTATION_PLAN.md`; the 41
implementation tickets live in `TICKETS.md` (and are also filed as
GitHub issues).

## Non-goals

Bithound is not trying to be:

- a Prometheus / Grafana replacement
- a general infrastructure monitoring agent
- a wallet
- a node management daemon
- an automated recovery system

Its scope is **observation + diagnosis + notification** for
Bitcoin-adjacent infrastructure. Action and visualization remain
the operator's responsibility.

## Repository layout

```text
SPEC.md                   Domain specification (sections 1–22)
docs/
├── src/adr/              Canonical home for all ADRs (rendered by mdBook)
├── INCIDENT_CATALOG.md   ~17 documented incident patterns (the diagnostic backlog)
└── OPERATOR_GUIDE.md     Operator-facing how-to for the V0 sidecar
IMPLEMENTATION_PLAN.md    Phases, milestones, dependency graph, estimates
TICKETS.md                JIRA-style tickets BTH-1 … BTH-58
examples/                 Copyable sample configs
src/                      Rust crate
tests/                    End-to-end integration tests (#[ignore]-gated)
migrations/               sqlx-managed SQLite schema
```

## Contributing

V0 tickets are tagged by phase, size, and priority — see the
[Issues](../../issues) page or filter by:

- `is:open label:phase:12` — current phase (end-to-end + docs)
- `is:open label:priority:high label:size:S` — small high-priority
  work
- `is:open label:size:L` — larger stories

Read `SPEC.md` and `IMPLEMENTATION_PLAN.md` before picking up a
ticket. Each issue references the ADRs it implements; deviations
require a new ADR under `docs/src/adr/` before merging.

## License

GNU GPLv3
