//! A person sees a run's nodes only with view on every current node; an unknown run is refused alike.

use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::authorization::main::row_in_target::row_in_target;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::runs::_helpers::lifecycle::reads::run_nodes_in;
use crate::runs::models::RunNodeRecord;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TargetRow, TransactionMode};

/// `None` when the run is not in this location and target; authorization runs first either way.
pub async fn list_person_run_nodes(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target, run_id): (Uuid, &str, Uuid),
    default_role: DefaultRole,
) -> Result<Option<Vec<RunNodeRecord>>, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, known) =
        row_in_target(transaction, (location_id, target), TargetRow::Run(run_id)).await?;
    if !known {
        commit(transaction).await?;
        return Ok(None);
    }
    let (transaction, nodes) = run_nodes_in(transaction, run_id).await?;
    commit(transaction).await?;
    Ok(Some(nodes))
}
