//! The newest result of every check in the target, read in the caller's transaction after its authorization.

use uuid::Uuid;

use crate::store::errors::ControlError;
use crate::store::models::ControlTransaction;
use crate::warehouse_history::_helpers::checks::latest_results;
use crate::warehouse_history::models::CheckResult;

pub(crate) async fn read_latest_checks(
    transaction: ControlTransaction,
    scope: (Uuid, &str),
) -> Result<(ControlTransaction, Vec<CheckResult>), ControlError> {
    let (transaction, rows) = latest_results(transaction, scope).await?;
    Ok((
        transaction,
        rows.into_iter()
            .map(|row| CheckResult::new(row.event_id, row.record, row.ignored))
            .collect(),
    ))
}
