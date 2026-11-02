//! Every normal store handle holds shared access; offline recovery obtains exclusive access.

use std::time::Duration;

use crate::store::_helpers::open_sqlite::{open_sqlite, sqlite_path};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

pub(crate) async fn open_control_store(
    url: &str,
    offline: bool,
) -> Result<ControlStore, ControlError> {
    if url.starts_with("postgres://") || url.starts_with("postgresql://") {
        let mut options = sqlx_postgres::PgPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(Duration::from_secs(30));
        if offline {
            options = options.max_connections(1);
        } else {
            options = options
                .min_connections(1)
                .idle_timeout(None)
                .max_lifetime(None)
                .after_connect(|connection, _| {
                    Box::pin(async move {
                        sqlx_core::query::query(
                        "SELECT pg_advisory_lock_shared(1936810573, hashtext(current_schema()))",
                    )
                    .execute(connection)
                    .await?;
                        Ok(())
                    })
                });
        }
        return Ok(ControlStore::Postgres(options.connect(url).await?));
    }
    let path = sqlite_path(url)?;
    tokio::task::spawn_blocking(move || open_sqlite(path, offline)).await?
}
