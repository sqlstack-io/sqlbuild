//! Commit facts and their projections together.

use crate::store::errors::ControlError;
use crate::store::models::{Connection, ControlTransaction};

pub(crate) async fn commit(mut transaction: ControlTransaction) -> Result<(), ControlError> {
    match transaction
        .connection
        .take()
        .ok_or_else(|| ControlError::Invalid("transaction has already finished".into()))?
    {
        Connection::Sqlite(connection) => {
            tokio::task::spawn_blocking(move || connection.execute_batch("COMMIT")).await??;
        }
        Connection::Postgres(transaction) => transaction.commit().await?,
    }
    Ok(())
}
