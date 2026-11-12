use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;

pub async fn connect(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let is_memory = url.contains(":memory:") || url.contains("mode=memory");

    let mut options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .foreign_keys(true);

    if !is_memory {
        options = options.journal_mode(SqliteJournalMode::Wal);
    }

    // An in-memory database lives inside a single connection, so pooling more
    // than one connection would give each request its own empty database.
    let max_connections = if is_memory { 1 } else { 5 };

    SqlitePoolOptions::new()
        .max_connections(max_connections)
        .connect_with(options)
        .await
}

pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

/// Creates the parent directory of a `sqlite://path` URL so the database file
/// can be created on first run.
pub fn ensure_database_dir(url: &str) -> Result<(), std::io::Error> {
    let Some(path) = url
        .strip_prefix("sqlite://")
        .or_else(|| url.strip_prefix("sqlite:"))
    else {
        return Ok(());
    };

    let path = path.split('?').next().unwrap_or(path);
    if path.is_empty() || path == ":memory:" {
        return Ok(());
    }

    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    Ok(())
}

pub async fn get_state(pool: &SqlitePool, key: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT value FROM app_state WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

pub async fn clear_state(pool: &SqlitePool, key: &str) {
    let _ = sqlx::query("DELETE FROM app_state WHERE key = ?")
        .bind(key)
        .execute(pool)
        .await;
}

pub async fn set_state(pool: &SqlitePool, key: &str, value: &str) {
    let result = sqlx::query(
        "INSERT INTO app_state (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await;

    if let Err(error) = result {
        tracing::warn!(%error, key, "failed to persist app state");
    }
}
