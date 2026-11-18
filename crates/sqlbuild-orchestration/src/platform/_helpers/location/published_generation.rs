//! Connection and warehouse sync health name a generation the server published.

use uuid::Uuid;

use crate::platform::models::{LocationHealth, LocationHealthReport};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// Refuse connection or warehouse sync health whose generation was never published.
pub(crate) async fn require_published_generation(
    transaction: ControlTransaction,
    location_id: Uuid,
    report: &LocationHealthReport,
) -> Result<ControlTransaction, ControlError> {
    let (LocationHealth::Connection { generation, .. }
    | LocationHealth::WarehouseSync { generation, .. }) = report.health
    else {
        return Ok(transaction);
    };
    let (transaction, snapshot) = query(
        transaction,
        "SELECT snapshot_id FROM sqb_snapshots WHERE location_id=$1 AND target=$2 AND generation=$3",
        vec![
            Parameter::Id(location_id),
            Parameter::Text(report.target.clone()),
            Parameter::Integer(generation),
        ],
    )
    .await?;
    if snapshot.is_empty() {
        let message = if matches!(report.health, LocationHealth::WarehouseSync { .. }) {
            "warehouse sync health requires a published generation"
        } else {
            "connection health requires a published generation"
        };
        return Err(ControlError::Invalid(message.into()));
    }
    Ok(transaction)
}
