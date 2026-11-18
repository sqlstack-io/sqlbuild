//! People read how many records the server mirrors of each state table, with the same view as the target's health.

use std::collections::BTreeMap;

use serde_json::Value;
use uuid::Uuid;

use crate::authorization::main::decisions::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::platform::models::MirroredTotal;
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};
use crate::warehouse_metadata::models::WarehouseHistoryKind;

/// Every mirrored state table of the target, in sync order, with the records held for it across its state locations.
pub async fn read_person_mirrored_totals(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    (location_id, target): (Uuid, &str),
    default_role: DefaultRole,
) -> Result<Vec<MirroredTotal>, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let counts = WarehouseHistoryKind::ALL
        .iter()
        .map(|kind| {
            format!(
                "SELECT '{}' AS kind, COUNT(*) AS records FROM {} WHERE location_id=$1 AND target=$2",
                kind.as_str(),
                kind.mirror_table()
            )
        })
        .collect::<Vec<_>>()
        .join(" UNION ALL ");
    let (transaction, rows) = query(
        transaction,
        &counts,
        vec![Parameter::Id(location_id), Parameter::Text(target.into())],
    )
    .await?;
    commit(transaction).await?;
    Ok(WarehouseHistoryKind::ALL
        .iter()
        .map(|kind| MirroredTotal {
            table: *kind,
            records: recorded(&rows, *kind),
        })
        .collect())
}

/// The records mirrored of `kind`, or none when the count names no row of it.
fn recorded(rows: &[BTreeMap<String, Value>], kind: WarehouseHistoryKind) -> u64 {
    rows.iter()
        .find(|row| row["kind"].as_str() == Some(kind.as_str()))
        .and_then(|row| row["records"].as_u64())
        .unwrap_or_default()
}
