//! Runtime layer: spawns the per-collector task tree, the central
//! pipeline consumer, and the notification dispatch worker, then
//! drives shutdown.

pub mod bootstrap;
pub mod supervisor;
