//! Generic telemetry primitives shared across domains.
//!
//! See [`probe`] for the building blocks: [`Probe`], [`ProbeConfig`],
//! [`ProbeSnapshot`], [`spawn_probe`], [`evaluate_ttl`].

mod probe;

pub use probe::*;
