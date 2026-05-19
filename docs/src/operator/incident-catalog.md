# Incident catalog

Bithound ships with a curated catalog of built-in incident kinds. Every
incident raised by the sidecar belongs to one of these kinds (or to a
custom kind you register — see [Custom incidents](custom-incidents.md)).

## Built-in V0 kinds

| Name                          | Subject       | Dimension       |
| ----------------------------- | ------------- | --------------- |
| `bitcoin.tip_lag`             | `BitcoinNode` | —               |
| `bitcoin.ibd_stall`           | `BitcoinNode` | —               |
| `bitcoin.peer_starvation`     | `BitcoinNode` | —               |
| `bitcoin.mempool_full`        | `BitcoinNode` | —               |
| `bitcoin.reorg_deep`          | `BitcoinNode` | —               |
| `host.disk_exhaustion`        | `Host`        | `mount_path`    |
| `lnd.channel_inactive`        | `LndChannel`  | —               |
| `lnd.htlc_stuck`              | `LndChannel`  | `payment_hash`  |
| `sidecar.collector_failing`   | `Host`        | `collector_id`  |

The authoritative source is
[`config/default_kinds.toml`](https://github.com/bithoundhq/bithound/blob/main/config/default_kinds.toml).
A build-time parity test keeps this list and the in-code constants in
sync; if the table above drifts, that's a documentation bug.

## What each field means

- **Subject** — the [`EntitySubjectKind`](../reference/incident-kinds.md)
  the kind operates on. Drafts that target a different subject are
  rejected at receipt time without mutating any incident state.
- **Dimension** — the per-instance sub-key. `host.disk_exhaustion` uses
  `mount_path` so the same host can have one open incident per mount;
  `lnd.htlc_stuck` uses `payment_hash` for the same reason. Kinds with
  no dimension dedup by subject alone.

## Diagnostic backlog

For the longer list of incident patterns we plan to detect (with
symptom / signals / diagnosis / action / look-alikes for each), see
[`docs/INCIDENT_CATALOG.md`](https://github.com/bithoundhq/bithound/blob/main/docs/INCIDENT_CATALOG.md)
in the repository.
