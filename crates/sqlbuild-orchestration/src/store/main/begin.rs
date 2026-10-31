//! Serialize durable appends before allocating their storage IDs.

use std::time::Duration;

use chrono::Utc;
use rusqlite::OpenFlags;

use crate::store::errors::ControlError;
use crate::store::main::refresh_clock::refresh_clock;
use crate::store::models::{Connection, ControlStore, ControlTransaction, TransactionMode};

pub(crate) async fn begin(
    store: &ControlStore,
    mode: TransactionMode,
) -> Result<ControlTransaction, ControlError> {
    let connection = match store {
        ControlStore::Sqlite(path) => {
            let path = path.clone();
            Connection::Sqlite(
                tokio::task::spawn_blocking(move || {
                    let connection = rusqlite::Connection::open_with_flags(
                        path,
                        OpenFlags::SQLITE_OPEN_READ_WRITE
                            | OpenFlags::SQLITE_OPEN_CREATE
                            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
                    )?;
                    connection.busy_timeout(Duration::from_secs(30))?;
                    connection.execute_batch(if mode == TransactionMode::Write {
                        "PRAGMA foreign_keys=ON; BEGIN IMMEDIATE"
                    } else {
                        "PRAGMA foreign_keys=ON; PRAGMA query_only=ON; BEGIN"
                    })?;
                    Ok::<_, ControlError>(connection)
                })
                .await??,
            )
        }
        ControlStore::Postgres(pool) => {
            let mut transaction = pool.begin().await?;
            if mode == TransactionMode::Write {
                sqlx_core::query::query("SELECT pg_advisory_xact_lock(1936810572, 1)")
                    .execute(&mut *transaction)
                    .await?;
            } else {
                sqlx_core::query::query(
                    "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY",
                )
                .execute(&mut *transaction)
                .await?;
            }
            Connection::Postgres(transaction)
        }
    };
    let transaction = ControlTransaction {
        connection: Some(connection),
        now: Utc::now(),
    };
    refresh_clock(transaction).await
}
