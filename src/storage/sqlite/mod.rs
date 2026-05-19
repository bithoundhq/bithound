//! SQLite backend per ADR-P1.

pub mod incident_repository;
pub mod observation_store;

pub use incident_repository::SqliteIncidentRepository;
pub use observation_store::SqliteObservationStore;

use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};

/// Open the SQLite pool that backs all repositories.
///
/// Sets `journal_mode = WAL` and `synchronous = NORMAL`, then applies any
/// pending migrations from `./migrations`. Idempotent on existing files.
///
/// Per ADR-P1: WAL allows concurrent readers alongside the single writer
/// established by ADR-S1; `synchronous = NORMAL` trades a small durability
/// window for ~3-10x faster writes — acceptable because observations are
/// re-derivable from collectors on the next tick.
pub async fn open_pool(path: &Path) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    /// Returns the set of user table names defined in the connected DB.
    async fn user_tables(pool: &SqlitePool) -> Vec<String> {
        let rows = sqlx::query(
            "SELECT name FROM sqlite_master \
             WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '\\_sqlx%' ESCAPE '\\'",
        )
        .fetch_all(pool)
        .await
        .expect("query sqlite_master");
        rows.into_iter()
            .map(|r| r.get::<String, _>("name"))
            .collect()
    }

    #[tokio::test]
    async fn open_pool_creates_schema_on_fresh_db() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test.db");
        let pool = open_pool(&path).await.expect("open_pool");

        let mut tables = user_tables(&pool).await;
        tables.sort();
        assert_eq!(
            tables,
            vec![
                "incidents".to_string(),
                "observations".to_string(),
                "suppression_rules".to_string(),
            ]
        );
    }

    #[tokio::test]
    async fn open_pool_is_idempotent_on_existing_db() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test.db");

        let pool1 = open_pool(&path).await.expect("first open");
        drop(pool1);

        let pool2 = open_pool(&path).await.expect("second open");
        let tables = user_tables(&pool2).await;
        assert!(tables.contains(&"observations".to_string()));
        assert!(tables.contains(&"incidents".to_string()));
        assert!(tables.contains(&"suppression_rules".to_string()));
    }

    #[tokio::test]
    async fn pragmas_are_applied() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("test.db");
        let pool = open_pool(&path).await.expect("open_pool");

        let journal: String = sqlx::query("PRAGMA journal_mode")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get::<String, _>(0);
        assert_eq!(journal.to_lowercase(), "wal");

        let synchronous: i64 = sqlx::query("PRAGMA synchronous")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get::<i64, _>(0);
        // NORMAL = 1.
        assert_eq!(synchronous, 1);
    }
}
