//! Refresh the authoritative database clock after taking transaction locks.

use chrono::DateTime;

use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{Connection, ControlTransaction};

pub(crate) async fn refresh_clock(
    transaction: ControlTransaction,
) -> Result<ControlTransaction, ControlError> {
    let sql = match transaction.connection {
        Some(Connection::Postgres(_)) => "SELECT clock_timestamp() AS now",
        _ => "SELECT strftime('%Y-%m-%dT%H:%M:%f000Z','now') AS now",
    };
    let (mut transaction, rows) = query(transaction, sql, vec![]).await?;
    let now = rows
        .first()
        .and_then(|row| row.get("now"))
        .and_then(|value| value.as_str())
        .ok_or_else(|| ControlError::Invalid("database clock did not return a timestamp".into()))?;
    transaction.now = DateTime::parse_from_rfc3339(now)
        .map_err(|error| ControlError::Invalid(error.to_string()))?
        .to_utc();
    Ok(transaction)
}
