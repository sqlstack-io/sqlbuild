//! Open a control database; no Python or warehouse connection is involved.

use std::time::Duration;

use crate::store::_helpers::open_sqlite::{open_sqlite, sqlite_path};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// Open PostgreSQL by URL, or SQLite by an explicit filesystem path.
pub async fn open_store(url: &str) -> Result<ControlStore, ControlError> {
    if url.starts_with("postgres://") || url.starts_with("postgresql://") {
        let pool = sqlx_postgres::PgPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(Duration::from_secs(30))
            .connect(url)
            .await?;
        return Ok(ControlStore::Postgres(pool));
    }
    let path = sqlite_path(url)?;
    let configure_path = path.clone();
    tokio::task::spawn_blocking(move || open_sqlite(configure_path)).await??;
    Ok(ControlStore::Sqlite(path))
}
