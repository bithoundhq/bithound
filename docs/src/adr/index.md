# Architecture decision records

ADRs are the durable record of design decisions in Bithound. The full
set is maintained in `SPEC.md` § 23; the index below is a navigation
aid that links into the canonical text.

> Until the ADRs are folded into this site one-by-one, follow the
> links into `SPEC.md` on GitHub.

## Clusters

- **Core.** ADR-001 — incident shape and identity.
- **L-cluster — incident lifecycle.**
  L1 (fingerprinting & kind registry), L2 (signal-to-incident lift
  policy), L3 (severity & escalation), L4 (single-writer engine),
  L5 (notifier-side suppression).
- **R-cluster — read models.** R1, R2, R3.
- **C-cluster — collectors.** C1, C2, C3.
- **S-cluster — runtime supervision.** S1, S2, S3.
- **P-cluster — persistence.** P1, P2, P3.
- **N-cluster — notifications.** N1, N2.
- **A-cluster — alerting & audit.** A1.
- **X-cluster — extensibility.** X1.

The full text lives in
[`SPEC.md`](https://github.com/bithoundhq/bithound/blob/main/SPEC.md).

## When to write a new ADR

If a ticket forces a decision the existing ADRs don't cover, **stop
and write a new ADR** before coding. Open a PR with the spec change
first; reviewers reject implementation PRs that extend the
architecture by precedent.
