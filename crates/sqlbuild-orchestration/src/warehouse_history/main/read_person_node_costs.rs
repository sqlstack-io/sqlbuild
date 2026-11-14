//! One node's estimated query cost in each recent build, newest first.

use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};
use crate::warehouse_history::_helpers::costs::{latest_costs_in, node_builds};
use crate::warehouse_history::models::NodeCostRecord;

/// The node's cost in each build that started in the last `days` days, at most `limit`.
pub async fn read_person_node_costs(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target, node_key): (Uuid, &str, &str),
    (days, limit, default_role): (u32, u32, DefaultRole),
) -> Result<Vec<NodeCostRecord>, ControlError> {
    let Some((kind, name)) = node_key.split_once(':') else {
        return Err(ControlError::Invalid(
            "node key must be <kind>:<name>".into(),
        ));
    };
    let since = Utc::now() - Duration::days(i64::from(days));
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, costs) = latest_costs_in(
        transaction,
        (location_id, target),
        since,
        Some((kind, name)),
    )
    .await?;
    commit(transaction).await?;
    Ok(node_builds(&costs, limit))
}
