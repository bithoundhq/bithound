//! Storage layer.
//!
//! `sqlite::open_pool` is the entry point used by the runtime to obtain a
//! configured `SqlitePool` (WAL + NORMAL + migrations applied). Concrete
//! repository implementations live under [`sqlite`].

pub mod sqlite;
pub mod traits;

pub use traits::{ObservationStore, StoreError};
