//! Execute inside an owned transaction, without blocking an async runtime on SQLite.

use crate::store::_helpers::database::{postgres_query, sqlite_query};
use crate::store::errors::ControlError;
use crate::store::models::{Connection, ControlTransaction, Parameter};
use crate::store::types::Row;

pub(crate) async fn query(
    mut transaction: ControlTransaction,
    sql: &str,
    parameters: Vec<Parameter>,
) -> Result<(ControlTransaction, Vec<Row>), ControlError> {
    let connection = transaction
        .connection
        .take()
        .ok_or_else(|| ControlError::Invalid("transaction has already finished".into()))?;
    let rows: Vec<Row> = match connection {
        Connection::Sqlite(connection) => {
            let sql = sql.to_owned();
            let (connection, result) = tokio::task::spawn_blocking(move || {
                let result = sqlite_query(&connection, &sql, &parameters);
                (connection, result)
            })
            .await?;
            transaction.connection = Some(Connection::Sqlite(connection));
            result?
        }
        Connection::Postgres(mut connection) => {
            let result = postgres_query(&mut connection, sql, parameters).await;
            transaction.connection = Some(Connection::Postgres(connection));
            result?
        }
    };
    Ok((transaction, rows))
}
