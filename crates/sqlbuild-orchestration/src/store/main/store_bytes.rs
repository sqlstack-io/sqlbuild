//! `store_bytes`: how large the control store's database is on disk.

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{Connection, ControlTransaction};

/// SQLite's main file without its write-ahead log, or the whole PostgreSQL database.
pub(crate) async fn store_bytes(
    transaction: ControlTransaction,
) -> Result<(ControlTransaction, i64), ControlError> {
    let sql = if matches!(transaction.connection, Some(Connection::Postgres(_))) {
        "SELECT pg_database_size(current_database()) AS bytes"
    } else {
        "SELECT page_count * page_size AS bytes FROM pragma_page_count(), pragma_page_size()"
    };
    let (transaction, rows) = query(transaction, sql, Vec::new()).await?;
    let bytes = rows
        .first()
        .and_then(|row| row["bytes"].as_i64())
        .ok_or_else(|| ControlError::Invalid("control store size is unavailable".into()))?;
    Ok((transaction, bytes))
}
