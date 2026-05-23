# Summary

[Introduction](README.md)

# Operator guide

- [Installation](operator/install.md)
- [Configuration](operator/configuration.md)
- [Incident catalog](operator/incident-catalog.md)
- [Custom incidents](operator/custom-incidents.md)
- [Notifications](operator/notifications.md)

# Contributor guide

- [Overview](contributor/overview.md)
- [Architecture](contributor/architecture.md)
- [Ticket workflow](contributor/workflow.md)

# Reference

- [Incident-kind schema](reference/incident-kinds.md)
- [Observation payloads](reference/observation-payloads.md)
- [Configuration schema](reference/config-schema.md)

# Architecture decision records

- [Index](adr/index.md)
- [ADR-001 — Incident-engine small calls](adr/001.md)
- [ADR-L1 — Incident fingerprinting & kind registry](adr/l1.md)
- [ADR-L2 — Signal-to-incident lift policy](adr/l2.md)
- [ADR-L3 — Severity & escalation semantics](adr/l3.md)
- [ADR-L4 — Engine surface area](adr/l4.md)
- [ADR-L5 — Suppression model](adr/l5.md)
- [ADR-R1 — Read-model architecture](adr/r1.md)
- [ADR-R2 — Derived observations as ObservationPayload variants](adr/r2.md)
- [ADR-R3 — Store small calls](adr/r3.md)
- [ADR-C1 — Two collector traits](adr/c1.md)
- [ADR-C2 — Polling output is ObservationBatch directly](adr/c2.md)
- [ADR-C3 — Collector small calls](adr/c3.md)
- [ADR-C4 — ZMQ subscription collector (Deferred to v1.0)](adr/c4.md)
- [ADR-S1 — Per-collector tasks + central consumer](adr/s1.md)
- [ADR-S2 — Per-batch rule evaluation](adr/s2.md)
- [ADR-S3 — Runtime small calls](adr/s3.md)
- [ADR-P1 — SQLite backend via sqlx](adr/p1.md)
- [ADR-P2 — Storage trait shapes and impl sketches](adr/p2.md)
- [ADR-P3 — Notification attempts persistence](adr/p3.md)
- [ADR-X1 — Single bithound.toml, env-var overrides only for secrets](adr/x1.md)
- [ADR-D1 — Unvalidated vs validated incident signal draft](adr/d1.md)
- [ADR-D2 — Smart constructors for name newtypes](adr/d2.md)
- [ADR-D3 — Full command vocabulary](adr/d3.md)
- [ADR-D4 — Cross-context domain events](adr/d4.md)
- [ADR-N1 — Identity refinements](adr/n1.md)
- [ADR-N2 — Notification delivery worker](adr/n2.md)
- [ADR-A1 — Local operator HTTP API](adr/a1.md)
