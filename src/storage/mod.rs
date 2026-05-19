//! Storage layer per ADR-P1, ADR-P2.
//!
//! `sqlite::open_pool` is the entry point used by the runtime to obtain a
//! configured `SqlitePool` (WAL + NORMAL + migrations applied). Concrete
//! repository implementations live under [`sqlite`].

pub mod sqlite;
