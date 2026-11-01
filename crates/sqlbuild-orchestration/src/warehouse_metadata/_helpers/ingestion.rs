//! A metadata page shares one registered session, snapshot fence and physical identity scope.

use uuid::Uuid;

use crate::platform::main::require_instance::require_instance;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, query::query};
use crate::store::models::{ControlStore, ControlTransaction, Parameter, TransactionMode};

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
