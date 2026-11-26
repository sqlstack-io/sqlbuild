//! A run's estimated query cost: the latest observation of each resource and warehouse it used.

use uuid::Uuid;

use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlStore, Parameter};
use crate::warehouse_history::_helpers::costs::RUN_COST_SQL;
use crate::warehouse_history::_helpers::run_recall::begin_run_recall;
use crate::warehouse_metadata::models::WarehouseResourceCostRecord;

/// `None` when the run is not in this location and target; empty before any cost was synced.
pub async fn read_person_run_cost(
    store: &ControlStore,
    credentials: (&AuditContext, Uuid),
    run: (Uuid, &str, Uuid),
    default_role: DefaultRole,
) -> Result<Option<Vec<WarehouseResourceCostRecord>>, ControlError> {
    let Some((transaction, _, _)) =
        begin_run_recall(store, credentials, run, (default_role, None)).await?
    else {
        return Ok(None);
    };
    let (location_id, target, run_id) = run;
    let (transaction, rows) = query(
        transaction,
        RUN_COST_SQL,
        vec![
            Parameter::Id(location_id),
            Parameter::Text(target.into()),
            Parameter::Text(run_id.to_string()),
        ],
    )
    .await?;
    commit(transaction).await?;
    rows.into_iter()
        .map(|row| {
            serde_json::from_value(row["record_json"].clone())
                .map_err(|error| ControlError::Invalid(error.to_string()))
        })
        .collect::<Result<_, _>>()
        .map(Some)
}
