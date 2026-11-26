//! Migration 0005's data step: snapshot `triggers` documents lose sensors stored before sensors were functions.

use std::future::Future;
use std::pin::Pin;

use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;
use crate::store_migrations::_helpers::trigger_documents::rewrite_trigger_documents;

/// Rewrite each stored `triggers` document without its old-form sensors, which have neither a `function` nor a `schedule`; their `sqb_triggers` rows stay, hidden by the migration's SQL.
pub(crate) fn drop_old_form_sensors(
    transaction: ControlTransaction,
) -> Pin<Box<dyn Future<Output = Result<ControlTransaction, ControlError>> + Send>> {
    Box::pin(rewrite_documents(transaction))
}

async fn rewrite_documents(
    transaction: ControlTransaction,
) -> Result<ControlTransaction, ControlError> {
    rewrite_trigger_documents(transaction, |entries| {
        let before = entries.len();
        entries
            .retain(|_, entry| entry.get("function").is_some() || entry.get("schedule").is_some());
        Ok(entries.len() != before)
    })
    .await
}
