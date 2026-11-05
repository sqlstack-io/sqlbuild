//! Run one operation in its own writer transaction and commit only when it succeeds.

use std::future::Future;

use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, ControlTransaction, TransactionMode};

pub(crate) async fn write<T, F, Fut>(store: &ControlStore, work: F) -> Result<T, ControlError>
where
    F: FnOnce(ControlTransaction) -> Fut,
    Fut: Future<Output = Result<(ControlTransaction, T), ControlError>>,
{
    let (transaction, value) = work(begin(store, TransactionMode::Write).await?).await?;
    commit(transaction).await?;
    Ok(value)
}
