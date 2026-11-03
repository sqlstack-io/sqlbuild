//! Restore structural catalog views without changing scan receipts or source facts.

use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::{Change, ControlTransaction, Parameter};
use crate::warehouse_metadata::_helpers::catalog::project_catalog_record;
use crate::warehouse_metadata::models::CatalogReplayRow;

pub(crate) async fn replay_catalog_projection(
    transaction: ControlTransaction,
) -> Result<ControlTransaction, ControlError> {
    let (mut transaction, _) =
        query(transaction, "DELETE FROM sqb_warehouse_catalog", vec![]).await?;
    let mut after = 0;
    loop {
        let rows;
        (transaction, rows) = query(transaction,
            "SELECT storage_id,event_schema_version AS schema_version,occurred_at,payload FROM recall_events \
             WHERE producer='warehouse' AND event_type='warehouse_catalog_changed' AND storage_id>$1 \
             ORDER BY storage_id LIMIT 100", vec![Parameter::Integer(after)]).await?;
        if rows.is_empty() {
            break;
        }
        for row in rows {
            let row: CatalogReplayRow = decode_row(row)?;
            if row.schema_version != 1 {
                return Err(ControlError::Invalid(
                    "cannot replay an unsupported catalog fact schema".into(),
                ));
            }
            after = row.storage_id;
            transaction.now = row.occurred_at;
            let fact = row.payload;
            if fact.present {
                transaction = project_catalog_record(
                    transaction,
                    (fact.location_id, &fact.target),
                    fact.relation_id,
                    &fact.record,
                )
                .await?;
            } else {
                (transaction, _) = query(
                    transaction,
                    "DELETE FROM sqb_warehouse_catalog WHERE event_id=$1",
                    vec![Parameter::Id(fact.relation_id)],
                )
                .await?;
            }
            transaction
                .changes
                .insert(Change::NodeStatus(fact.location_id, fact.target));
        }
    }
    Ok(transaction)
}
