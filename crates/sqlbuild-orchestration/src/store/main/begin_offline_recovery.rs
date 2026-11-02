//! Fail closed if any supported store client still holds access to this installation.

use crate::store::_helpers::open_control_store::open_control_store;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, query::query};
use crate::store::models::{ControlStore, OfflineRecovery, TransactionMode};

pub async fn begin_offline_recovery(url: &str) -> Result<OfflineRecovery, ControlError> {
    let store = open_control_store(url, true).await?;
    let transaction = begin(&store, TransactionMode::Write).await?;
    if matches!(store, ControlStore::Postgres(_)) {
        let (transaction, rows) = query(transaction,
            "SELECT pg_try_advisory_xact_lock(1936810573, hashtext(current_schema())) AS exclusive_access",
            vec![]).await?;
        if rows
            .first()
            .is_none_or(|row| row["exclusive_access"] != true)
        {
            return Err(ControlError::Invalid(
                "offline recovery requires exclusive control-store access; stop all server and administration processes".into(),
            ));
        }
        return Ok(OfflineRecovery(transaction));
    }
    Ok(OfflineRecovery(transaction))
}
