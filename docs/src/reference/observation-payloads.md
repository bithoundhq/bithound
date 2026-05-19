# Observation payloads

> **Stub.** A full table of payload variants will live here once the
> last few variants stabilise.

Bithound's observation envelope carries one of ten payload variants:

- State observations (one variant per `StateName`).
- Metric observations.
- Health check observations.
- Heartbeat observations.
- Capability observations.
- Incident signal observations.
- Event observations.
- Transition observations.
- Inventory observations.
- Diagnosis observations.

See [`src/observations/types/`](https://github.com/bithoundhq/bithound/tree/main/src/observations/types)
for the current shapes; each variant has unit tests that exercise
serde round-trips.
