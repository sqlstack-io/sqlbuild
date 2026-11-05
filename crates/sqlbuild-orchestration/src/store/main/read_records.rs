//! Read typed records from one consistent read transaction.

use serde::de::DeserializeOwned;

use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query_records::query_records};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

pub(crate) async fn read_records<T: DeserializeOwned>(
    store: &ControlStore,
    sql: &str,
    parameters: Vec<Parameter>,
) -> Result<Vec<T>, ControlError> {
    let transaction = begin(store, TransactionMode::Read).await?;
    let (transaction, records) = query_records(transaction, sql, parameters).await?;
    commit(transaction).await?;
    Ok(records)
}
