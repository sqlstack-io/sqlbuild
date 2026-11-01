//! Serialize durable appends before allocating their storage IDs.

use std::time::Duration;

use chrono::Utc;
use rusqlite::OpenFlags;

use crate::store::_helpers::changes::sqlite_changes;
use crate::store::errors::ControlError;
use crate::store::main::refresh_clock::refresh_clock;
use crate::store::models::{Connection, ControlStore, ControlTransaction, TransactionMode};

pub(crate) async fn begin(
    store: &ControlStore,
    mode: TransactionMode,
) -> Result<ControlTransaction, ControlError> {
    let (connection, notifications) = match store {
        ControlStore::Sqlite(path) => {
            let path = path.clone();
            let (connection, notifications) = tokio::task::spawn_blocking(move || {
                let connection = rusqlite::Connection::open_with_flags(
                    &path,
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
                let notifications = if mode == TransactionMode::Write {
                    Some(sqlite_changes(&path)?)
                } else {
                    None
                };
                Ok::<_, ControlError>((connection, notifications))
            })
            .await??;
            (Connection::Sqlite(connection), notifications)
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
            (Connection::Postgres(transaction), None)
        }
    };
    let transaction = ControlTransaction {
        connection: Some(connection),
        now: Utc::now(),
        changes: Default::default(),
        notifications,
    };
    refresh_clock(transaction).await
}
