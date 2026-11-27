//! Open a control database without Python or warehouse connections.

use std::time::Duration;

use crate::store::_helpers::open_sqlite::{open_sqlite, sqlite_path};
use crate::store::constants::{
    IDLE_IN_TRANSACTION_TIMEOUT, POOL_CONNECTIONS, POOL_WAIT_SECONDS, STATEMENT_TIMEOUT,
};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// Open PostgreSQL by URL, or SQLite by an explicit filesystem path; every pooled PostgreSQL session is held to the server's limits.
pub async fn open_store(url: &str) -> Result<ControlStore, ControlError> {
    if url.starts_with("postgres://") || url.starts_with("postgresql://") {
        return Ok(ControlStore::Postgres(
            sqlx_postgres::PgPoolOptions::new()
                .max_connections(POOL_CONNECTIONS)
                .acquire_timeout(Duration::from_secs(POOL_WAIT_SECONDS))
                .after_connect(|connection, _| Box::pin(set_session_limits(connection)))
                .connect(url)
                .await?,
        ));
    }
    let path = sqlite_path(url)?;
    tokio::task::spawn_blocking(move || open_sqlite(path)).await?
}

/// Hold a new session to the statement and idle-in-transaction limits, so a stuck task never keeps a statement running or a transaction open.
async fn set_session_limits(
    connection: &mut sqlx_postgres::PgConnection,
) -> Result<(), sqlx_core::Error> {
    for (setting, limit) in [
        ("statement_timeout", STATEMENT_TIMEOUT),
        (
            "idle_in_transaction_session_timeout",
            IDLE_IN_TRANSACTION_TIMEOUT,
        ),
    ] {
        let statement = format!("SET {setting} = {}", limit.as_millis());
        sqlx_core::query::query(&statement)
            .execute(&mut *connection)
            .await?;
    }
    Ok(())
}
