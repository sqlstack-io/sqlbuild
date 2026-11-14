//! The container limits a job asked for in the snapshot its run is pinned to.

use uuid::Uuid;

use crate::declarations::models::{JobDefinition, RunResources};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};

/// `None` when the job asks for nothing or the snapshot no longer holds it.
pub async fn job_resources(
    store: &ControlStore,
    (snapshot_id, job_key): (Uuid, &str),
) -> Result<Option<RunResources>, ControlError> {
    let (transaction, rows) = query(
        begin(store, TransactionMode::Read).await?,
        "SELECT definition_json FROM sqb_jobs WHERE snapshot_id=$1 AND job_key=$2",
        vec![Parameter::Id(snapshot_id), Parameter::Text(job_key.into())],
    )
    .await?;
    commit(transaction).await?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    let definition: JobDefinition = serde_json::from_value(row["definition_json"].clone())
        .map_err(|error| ControlError::Schema(error.to_string()))?;
    Ok(definition.resources)
}
