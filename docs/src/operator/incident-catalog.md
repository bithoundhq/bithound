# Incident catalog

Bithound's incident-kind registry is the closed set of incident
"shapes" the engine knows how to open. V0 ships three rules wired
against three kinds; operators can extend the registry through
[Custom incidents](custom-incidents.md).

## Built-in V0 kinds

| Name                                | Subject       | Dimension | `min_open_confidence` |
| ----------------------------------- | ------------- | --------- | --------------------- |
| `bitcoin.rpc_unreachable`           | `BitcoinNode` | —         | `High`                |
| `bitcoin.no_peers`                  | `BitcoinNode` | —         | `High`                |
| `bitcoin.tip_lag_or_ibd_stalled`    | `BitcoinNode` | —         | `High`                |

The authoritative source is
[`config/default_kinds.toml`](https://github.com/bithoundhq/bithound/blob/main/config/default_kinds.toml).
A build-time parity test in
[`src/incidents/well_known.rs`](https://github.com/bithoundhq/bithound/blob/main/src/incidents/well_known.rs)
keeps this table and the in-code constants in sync; if the table
above drifts, that's a documentation bug worth a PR.

## What each rule fires on

### `bitcoin.rpc_unreachable`

**Fires when** all four Bitcoin RPC health-check targets
(`getblockchaininfo`, `getmempoolinfo`, `getnetworkinfo`,
`getpeerinfo`) report `HealthStatus::Critical` for ≥ 60 seconds.

**Clears when** any one of them returns to `HealthStatus::Ok`.

**What to do.** First check whether `bitcoind` is up. If it is,
check the RPC port and auth — that's the most common cause of a
sustained all-four-Critical outage.

Implementation:
[`src/diagnostics/rules/bitcoin/rpc_unreachable.rs`](https://github.com/bithoundhq/bithound/blob/main/src/diagnostics/rules/bitcoin/rpc_unreachable.rs).
This rule doesn't map to a catalog A-*/X-* entry — it's an
operability signal (the sidecar can't reach the node), not a
node-state pathology.

### `bitcoin.no_peers`

**Fires when** `getnetworkinfo.connections_out == 0` AND
`networkactive == true` continuously for ≥ 60 seconds.

**Clears when** the outbound peer count returns to non-zero.

**Silent when** the operator has deliberately disabled networking
(`networkactive == false`). Tightens incident-catalog entry
[A3](https://github.com/bithoundhq/bithound/blob/main/docs/INCIDENT_CATALOG.md#a3-outbound-peer-starvation)
from the original "< 8 outbound" to the unambiguous zero case so the
V0 alert is high-signal.

**What to do.** Check firewall and port 8333 reachability. The node
may be partitioned, the ISP may be blocking, or addrman may have
churned through every known peer. Add manual peers via
`bitcoin-cli addnode` to known-good nodes while you investigate.

Implementation:
[`src/diagnostics/rules/bitcoin/no_peers.rs`](https://github.com/bithoundhq/bithound/blob/main/src/diagnostics/rules/bitcoin/no_peers.rs).

### `bitcoin.tip_lag_or_ibd_stalled`

**Fires when** *either* pattern below holds across two consecutive
polls:

- **A1 (tip lag)** — `initialblockdownload == true` AND
  `headers - blocks < 1000` AND `verificationprogress > 0.999` AND
  `peer_count ≥ 8`. The node thinks it's syncing but is effectively
  at the tip.
- **A2 (IBD stall)** — `headers - blocks ≥ 1000` AND
  `verification_progress` is flat (no change > 1e-9) across the
  last 5 minutes. The node is genuinely syncing but the download
  window has stalled.

**Clears when** neither pattern holds across two consecutive polls.

**What to do.** For the A1 shape, see
[catalog entry A1](https://github.com/bithoundhq/bithound/blob/main/docs/INCIDENT_CATALOG.md#a1-tip-lag--node-believes-it-is-in-ibd-when-it-shouldnt-be) —
usually a `-maxtipage` restart or `reconsiderblock`. For A2, see
[catalog entry A2](https://github.com/bithoundhq/bithound/blob/main/docs/INCIDENT_CATALOG.md#a2-ibd-stall--block-download-window-starvation) —
usually a peer-churn problem.

Implementation:
[`src/diagnostics/rules/bitcoin/tip_lag_or_ibd_stalled.rs`](https://github.com/bithoundhq/bithound/blob/main/src/diagnostics/rules/bitcoin/tip_lag_or_ibd_stalled.rs).

## How the registry validates drafts

Every signal a rule emits carries a `(subject, kind, dimension)`
fingerprint. On receipt, the engine looks the kind up in the
registry and rejects the draft if:

- **`UnknownKind`** — the kind isn't registered.
- **`DisallowedSubject`** — the subject's kind isn't in the
  registered `allowed_subjects` list.
- **`DimensionRequired`** — the kind has `allows_dimension = true`
  and the draft has no `dimension`.
- **`DimensionForbidden`** — the kind has `allows_dimension = false`
  and the draft has a `dimension`.

A rejected draft persists no signal observation and mutates no
incident state. See
[Reference → Incident-kind schema](../reference/incident-kinds.md)
for the per-error reference.

## Diagnostic backlog

V0 wires three rules; v0.0.8.0 added two more in-tree but not yet
wired into the runtime. The longer list of incident patterns we plan
to detect — with symptom / signals / diagnosis / action / look-alikes
for each — lives in
[`docs/INCIDENT_CATALOG.md`](https://github.com/bithoundhq/bithound/blob/main/docs/INCIDENT_CATALOG.md).
Catalog entries A1, A2, A3 are the three V0 entries wired to rules
and reachable from the running sidecar. Catalog entries B1 and B6
have rule implementations as of v0.0.8.0 (`lnd.channel_inactive`,
`lnd.chain_backend_lag`) but the runtime wiring lands in BTH-67
(Polar e2e) and BTH-68 — they don't fire in a running sidecar yet.
The remaining A4–A8, B2–B5, C1–C3, X1, X2 land in V0.9+.
