//! Raise a transaction's session limits, for the migrations and publications that legitimately run long.

use crate::store::constants::LONG_TRANSACTION_TIMEOUT;
use crate::store::errors::ControlError;
use crate::store::models::{Connection, ControlTransaction};

/// Let the rest of `transaction` run statements and pause between them up to the long limit, for a migration or the publication of a large snapshot.
pub(crate) async fn long_transaction(
    mut transaction: ControlTransaction,
) -> Result<ControlTransaction, ControlError> {
    let Some(Connection::Postgres(connection)) = transaction.connection.as_mut() else {
        return Ok(transaction);
    };
    let limit = LONG_TRANSACTION_TIMEOUT.as_millis();
    for setting in ["statement_timeout", "idle_in_transaction_session_timeout"] {
        let statement = format!("SET LOCAL {setting} = {limit}");
        sqlx_core::query::query(&statement)
            .execute(&mut **connection)
            .await?;
    }
    Ok(transaction)
}
