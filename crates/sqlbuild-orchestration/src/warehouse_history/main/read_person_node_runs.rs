//! A node's run history needs view on every current node, as each run's node list does.

use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::runs::models::NodeRunRecord;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_history::_helpers::node_runs::node_runs_in;

/// The runs that built one node in this target, newest first; a key no run built returns an empty list.
pub async fn read_person_node_runs(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target, node_key): (Uuid, &str, &str),
    (limit, default_role): (u32, DefaultRole),
) -> Result<Vec<NodeRunRecord>, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, runs) =
        node_runs_in(transaction, (location_id, target, node_key), limit).await?;
    commit(transaction).await?;
    Ok(runs)
}
