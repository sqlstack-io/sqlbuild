//! Open a control database without Python or warehouse connections.

use std::time::Duration;

use crate::store::_helpers::open_sqlite::{open_sqlite, sqlite_path};
use crate::store::constants::{POOL_CONNECTIONS, POOL_WAIT_SECONDS};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// Open PostgreSQL by URL, or SQLite by an explicit filesystem path.
pub async fn open_store(url: &str) -> Result<ControlStore, ControlError> {
    if url.starts_with("postgres://") || url.starts_with("postgresql://") {
        return Ok(ControlStore::Postgres(
            sqlx_postgres::PgPoolOptions::new()
                .max_connections(POOL_CONNECTIONS)
                .acquire_timeout(Duration::from_secs(POOL_WAIT_SECONDS))
                .connect(url)
                .await?,
        ));
    }
    let path = sqlite_path(url)?;
    tokio::task::spawn_blocking(move || open_sqlite(path)).await?
}
