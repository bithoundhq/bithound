# Observation payloads

The `Observation` envelope is the unit of work the pipeline routes.
Every observation carries:

- An `id` (UUIDv7) and `observed_at` timestamp.
- An `ObservationSource` — which sidecar + collector produced it.
- An `EntityRef` subject — which monitored entity it concerns.
- An `ObservationOrigin` — `Collected` (came from a collector) or
  `Computed` (synthesized by the engine, e.g. signal observations).
- One of ten payload variants from `ObservationPayload`.
- An open-ended `Attributes` map for collector-specific decorations.

All ten variants live under
[`src/observations/types/`](https://github.com/bithoundhq/bithound/tree/main/src/observations/types).
Each has its own unit tests that exercise serde round-trips, so a
breaking shape change shows up at compile time.

## The ten payload variants

| Variant | Type | What it carries | Notes |
| --- | --- | --- | --- |
| `State` | `StateObservation` | A structured snapshot of one named subsystem (8 sub-variants in V0). | The dominant payload for V0; see *State observations* below. |
| `Metric` | `MetricObservation` | A single numeric sample of a named series. | Sparse in V0; populated as rules ask for time-windowed metric queries. |
| `Health` | `HealthCheckObservation` | The result of an active probe against a named target (`HealthTargetId`). | V0 emits four per Bitcoin RPC poll. |
| `Heartbeat` | `HeartbeatObservation` | The sidecar's liveness signal. | Not yet emitted on a schedule in V0; the type is ready for the V0.1 heartbeat collector. |
| `Capability` | `CapabilityObservation` | A boolean "this feature is configured / advertised". | Used for rules like the planned `bitcoin.zmq_not_configured`. |
| `IncidentSignal` | `IncidentSignalObservation` | A rule's emit — `(signal_name, incident_kind, severity, status, confidence)`. | The engine writes these on every accepted draft (ADR-D4). |
| `Event` | `EventObservation` | A point-in-time named event with structured attributes. | Sparse in V0. |
| `Transition` | `TransitionObservation` | A `(name, from, to)` state-change record. | Sparse in V0. |
| `Inventory` | `InventoryObservation` | A "current list of X" snapshot (peers, channels, mounts). | Sparse in V0. |
| `Diagnosis` | `DiagnosisObservation` | A higher-level human-readable summary derived from other observations. | Sparse in V0. |

The variant order in the enum (`Capability`, `Diagnosis`, `Event`, …)
is alphabetical for stable serde discriminant ordering; the table
above groups by use-frequency in V0 instead.

## State observations

`StateObservation` is a single typed enum across eight subject types.
The `StateName` constants in
[`src/observations/types/state/well_known.rs`](https://github.com/bithoundhq/bithound/blob/main/src/observations/types/state/well_known.rs)
are kept in sync with the enum variants by a parity test in
`src/observations/types/state.rs`.

| `StateName` constant | Enum variant | Body |
| --- | --- | --- |
| `BITCOIN_BLOCKCHAIN` | `BitcoinBlockchain` | `BitcoinBlockchainState` — chain, blocks, headers, verification_progress, ibd, pruned, size_on_disk_bytes |
| `BITCOIN_MEMPOOL` | `BitcoinMempool` | `BitcoinMempoolState` — loaded, tx_count, bytes, usage_bytes, max_mempool_bytes |
| `BITCOIN_NETWORK` | `BitcoinNetwork` | `BitcoinNetworkState` — version, subversion, protocol_version, connections, in/out counts, network_active |
| `BITCOIN_PEER_SUMMARY` | `BitcoinPeerSummary` | `BitcoinPeerSummaryState` — peer_count, inbound/outbound, block-relay-only |
| `LND_NODE` | `LndNode` | `LndNodeState` — identity_pubkey, channel counts, peer count, block_height, synced_to_chain |
| `LND_WALLET` | `LndWallet` | `LndWalletState` — total / confirmed / unconfirmed balance in sats |
| `LND_CHANNEL_SUMMARY` | `LndChannelSummary` | `LndChannelSummaryState` — active/inactive/pending channel counts, total capacity, balances |
| `HOST_SYSTEM` | `Host` | `HostState` — hostname, os, kernel, uptime, cpu count, memory, disk |

V0 collectors emit the four Bitcoin sub-variants only. The LND and
host sub-variants ship as types so V0.1 collectors land without a
schema migration.

## Append-only invariant

Every observation is **immutable** once persisted. Rules and the
engine never modify an `Observation`; if a state changes, the next
collector batch carries a new observation with a new `id`. Consumers
that need the latest state ask the read-model layer, not the
observation store.

This is why ADR-R2 makes derived facts (incident signals, diagnoses,
etc.) full `ObservationPayload` variants rather than separate tables:
they're observations the engine produced, and they share the
append-only audit shape with collector observations.

## Where to read further

- [`src/observations/types/`](https://github.com/bithoundhq/bithound/tree/main/src/observations/types) —
  the canonical shapes, with serde round-trip tests per variant.
- [ADR-R2](../adr/r2.md) — why derived observations are
  `ObservationPayload` variants.
- [ADR-R1](../adr/r1.md) — the read-model trait surface that lets
  rules query the latest of each kind without scanning the store.
