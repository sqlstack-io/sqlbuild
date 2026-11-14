//! Commit facts and their projections together.

use crate::store::_helpers::changes::resolve_changes;
use crate::store::constants::LARGE_WAL_BYTES;
use crate::store::errors::ControlError;
use crate::store::models::{Connection, ControlTransaction};

pub(crate) async fn commit(transaction: ControlTransaction) -> Result<(), ControlError> {
    let (mut transaction, messages) = resolve_changes(transaction).await?;
    match transaction
        .connection
        .take()
        .ok_or_else(|| ControlError::Invalid("transaction has already finished".into()))?
    {
        Connection::Sqlite(connection) => {
            tokio::task::spawn_blocking(move || {
                connection.execute_batch("COMMIT")?;
                if transaction.notifications.is_some() {
                    drop(truncate_large_wal(&connection));
                }
                if let Some(sender) = transaction.notifications {
                    for message in messages {
                        let _ = sender.send(message);
                    }
                }
                Ok::<_, rusqlite::Error>(())
            })
            .await??;
        }
        Connection::Postgres(mut transaction) => {
            for message in messages {
                let payload = serde_json::to_string(&message)
                    .map_err(|error| ControlError::Invalid(error.to_string()))?;
                sqlx_core::query::query(
                    "SELECT pg_notify('sqb_changes_' || md5(current_schema()), $1)",
                )
                .bind(payload)
                .execute(&mut *transaction)
                .await?;
            }
            transaction.commit().await?;
        }
    }
    Ok(())
}

/// Reset a WAL a large transaction left, which slows reads, unless readers hold it; best effort.
fn truncate_large_wal(connection: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    let Some(database) = connection.path() else {
        return Ok(());
    };
    let large = std::fs::metadata(format!("{database}-wal"))
        .is_ok_and(|metadata| metadata.len() > LARGE_WAL_BYTES);
    if large {
        connection.busy_timeout(std::time::Duration::ZERO)?;
        connection.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
    }
    Ok(())
}
