//! SQLite shares an in-process channel; PostgreSQL listens across independent server replicas.

use tokio::sync::broadcast;

use crate::store::_helpers::changes::sqlite_changes;
use crate::store::errors::ControlError;
use crate::store::main::listen_changes::listen_changes;
use crate::store::models::{ChangeMessage, ControlStore};

pub(crate) async fn subscribe_changes(
    store: &ControlStore,
) -> Result<
    (
        broadcast::Sender<ChangeMessage>,
        Option<sqlx_postgres::PgListener>,
    ),
    ControlError,
> {
    match store {
        ControlStore::Sqlite(path, _) => {
            let path = path.clone();
            Ok((
                tokio::task::spawn_blocking(move || sqlite_changes(&path)).await??,
                None,
            ))
        }
        ControlStore::Postgres(pool) => {
            let listener = listen_changes(pool).await?;
            let (sender, _) = broadcast::channel(256);
            Ok((sender, Some(listener)))
        }
    }
}
