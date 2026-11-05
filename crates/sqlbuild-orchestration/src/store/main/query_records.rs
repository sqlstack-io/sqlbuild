//! Query inside the caller's transaction and decode every row as one typed record.

use serde::de::DeserializeOwned;

use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::{ControlTransaction, Parameter};

pub(crate) async fn query_records<T: DeserializeOwned>(
    transaction: ControlTransaction,
    sql: &str,
    parameters: Vec<Parameter>,
) -> Result<(ControlTransaction, Vec<T>), ControlError> {
    let (transaction, rows) = query(transaction, sql, parameters).await?;
    let records = rows.into_iter().map(decode_row).collect::<Result<_, _>>()?;
    Ok((transaction, records))
}
