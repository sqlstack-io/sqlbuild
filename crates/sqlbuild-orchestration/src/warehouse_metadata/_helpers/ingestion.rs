//! A metadata page shares one registered session, snapshot fence and physical identity scope.

use uuid::Uuid;

use crate::platform::main::require_instance::require_instance;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{Change, ControlStore, ControlTransaction, Parameter, TransactionMode};
use crate::warehouse_metadata::_helpers::reconciliation::finish_reconciliation;
use crate::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use crate::warehouse_metadata::models::{WarehouseReconciliationPage, WarehouseSyncResponse};

pub(crate) async fn begin_ingestion(
    store: &ControlStore,
    (location_id, target): (Uuid, &str),
    (session_id, generation): (Uuid, i64),
) -> Result<ControlTransaction, ControlError> {
    let transaction = require_instance(
        begin(store, TransactionMode::Write).await?,
        (location_id, session_id, target),
    )
    .await?;
    let (transaction, rows) = query(
        transaction,
        "SELECT s.snapshot_id FROM sqb_current_snapshots c JOIN sqb_snapshots s ON s.snapshot_id=c.snapshot_id \
         WHERE c.location_id=$1 AND c.target=$2 AND s.generation=$3",
        vec![Parameter::Id(location_id), Parameter::Text(target.into()), Parameter::Integer(generation)],
    ).await?;
    if rows.is_empty() {
        return Err(ControlError::Fenced);
    }
    Ok(transaction)
}

pub(crate) fn scoped_event_id(
    (location_id, target): (Uuid, &str),
    (database, schema, table): (Option<&str>, &str, &str),
    source_event_id: &str,
) -> Result<Uuid, ControlError> {
    let identity = serde_json::to_vec(&(target, database, schema, table, source_event_id))
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    Ok(Uuid::new_v5(&location_id, &identity))
}

pub(crate) async fn commit_ingestion(
    mut transaction: ControlTransaction,
    (table, location_id, target): (&str, Uuid, &str),
    (page, keys): (Option<&WarehouseReconciliationPage>, &[Uuid]),
    applied: bool,
) -> Result<WarehouseSyncResponse, ControlError> {
    let mut response = WarehouseSyncResponse {
        applied: false,
        next_reconciliation_after: None,
    };
    if let Some(page) = page {
        (transaction, response) =
            finish_reconciliation(transaction, (table, location_id, target), page, keys).await?;
    }
    response.applied |= applied;
    if response.applied {
        transaction
            .changes
            .insert(Change::NodeStatus(location_id, target.into()));
    }
    commit(transaction).await?;
    Ok(response)
}

pub(crate) fn validate_fields(
    (kind, generation, count): (&str, i64, usize),
    invalid_identity: bool,
    invalid_scope: bool,
) -> Result<(), ControlError> {
    if generation < 1 || count > MAX_HISTORY_RECORDS || invalid_identity {
        return Err(ControlError::Invalid(format!(
            "{kind} sync needs a positive generation, at most {MAX_HISTORY_RECORDS} records and nonempty state identities"
        )));
    }
    if invalid_scope {
        return Err(ControlError::Invalid(format!(
            "{kind} reconciliation records must belong to its physical namespace and UTC day"
        )));
    }
    Ok(())
}
