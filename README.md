# Bithound Sidecar

Bithound Sidecar is a work-in-progress telemetry agent for Bitcoin infrastructure.

The goal is to provide a lightweight process that runs next to a Bitcoin node, collects operational signals, and exposes them to local or remote consumers. It is currently experimental and not ready for production use.

## What it will do

Bithound Sidecar is intended to monitor Bitcoin node health and produce structured telemetry around:

- chain state
- network state
- peer health
- RPC availability
- node synchronization status
- process and host-level resource usage
- filesystem and disk pressure
- incident-relevant signals

The sidecar will initially focus on Bitcoin Core. Support for Elements/Liquid, Lightning nodes, and other infrastructure may be added later.

## Current status

This project is in early development.

The internal architecture is being designed around:

- probes that collect individual measurements
- probe runners that execute probes periodically
- observation streams for raw probe results
- reducers that maintain telemetry state
- snapshots that expose the latest known node state
- consumers/exporters that can publish metrics or detect incidents

The API, configuration format, metric names, and internal module layout are still subject to change.

## Intended use

Eventually, Bithound Sidecar should be usable as a small local agent that can:

- run alongside a node
- collect telemetry through RPC and local system APIs
- expose current node state
- feed dashboards, alerts, and incident detection
- integrate with the future Bithound Cloud service

## Non-goals for now

At this stage, Bithound Sidecar is not trying to be:

- a full replacement for Prometheus
- a full replacement for Grafana
- a generic infrastructure monitoring agent
- a wallet
- a node management daemon
- an automated recovery system

The first milestone is reliable Bitcoin node observability.

## License

GNU GPLv3
