# Architecture decision records

ADRs are Bithound's durable design record. Each one captures **Context**
(what triggered the decision), **Decision** (what we chose), **Rationale**
(why), and **Alternatives considered** (what we rejected). Decisions
override conflicting statements elsewhere in the documentation; when an
ADR lands, earlier docs are updated in place to match.

This site is the canonical home for ADRs. `SPEC.md` § 23 is a pointer
back here, not a duplicate copy.

## When to write a new ADR

If a ticket forces a decision the existing ADRs don't cover, **stop and
write a new ADR before coding**. Open a PR with the ADR first; reviewers
reject implementation PRs that extend the architecture by precedent.

ADR IDs are clustered by topic — pick the next ID in the relevant
cluster (e.g. the next persistence ADR is P4). A new cluster gets a new
letter; document its scope at the top of the first ADR in the cluster.

## Index

### Core

- [ADR-001 — Incident-engine small calls](001.md)

### L cluster — incident lifecycle

- [ADR-L1 — Incident fingerprinting & kind registry](l1.md)
- [ADR-L2 — Signal-to-incident lift policy](l2.md)
- [ADR-L3 — Severity & escalation semantics](l3.md)
- [ADR-L4 — Engine surface area](l4.md)
- [ADR-L5 — Suppression model](l5.md)

### R cluster — read models

- [ADR-R1 — Read-model architecture](r1.md)
- [ADR-R2 — Derived observations as `ObservationPayload` variants](r2.md)
- [ADR-R3 — Store small calls](r3.md)

### C cluster — collectors

- [ADR-C1 — Two collector traits](c1.md)
- [ADR-C2 — Polling output is `ObservationBatch` directly](c2.md)
- [ADR-C3 — Collector small calls](c3.md)
- [ADR-C4 — ZMQ subscription collector (Deferred to v1.0)](c4.md)

### S cluster — runtime supervision

- [ADR-S1 — Per-collector tasks + central consumer](s1.md)
- [ADR-S2 — Per-batch rule evaluation against the batch's subject](s2.md)
- [ADR-S3 — Runtime small calls](s3.md)

### P cluster — persistence

- [ADR-P1 — SQLite backend via sqlx for all three repositories](p1.md)
- [ADR-P2 — Storage trait shapes and impl sketches](p2.md)
- [ADR-P3 — Notification attempts persistence](p3.md)

### X cluster — extensibility

- [ADR-X1 — Single `bithound.toml`, env-var overrides only for secrets](x1.md)

### D cluster — domain modelling

- [ADR-D1 — Unvalidated vs validated incident signal draft](d1.md)
- [ADR-D2 — Smart constructors for name newtypes](d2.md)
- [ADR-D3 — Full command vocabulary (Incident + Suppression services)](d3.md)
- [ADR-D4 — Cross-context domain events (β: events-only output)](d4.md)

### N cluster — notifications

- [ADR-N1 — Identity refinements (Sidecar subject + sub-entity scoping)](n1.md)
- [ADR-N2 — Notification delivery worker (out of the central consumer)](n2.md)

### A cluster — alerting & operator API

- [ADR-A1 — Local operator HTTP API](a1.md)

### E cluster — Lightning-domain modelling

- [ADR-E1 — LND-domain state and kinds for V0.8](e1.md)
