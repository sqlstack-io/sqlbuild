//! Sequence preexisting observations inside migration's writer fence, before new imports.

use crate::store::errors::ControlError;
use crate::store::main::{decode_row::decode_row, query::query};
use crate::store::models::{ControlTransaction, Parameter};
use crate::warehouse_metadata::_helpers::freshness::append_observation;
use crate::warehouse_metadata::models::StoredSourceFreshnessObservation;

pub(crate) async fn bootstrap_source_observations(
    mut transaction: ControlTransaction,
) -> Result<ControlTransaction, ControlError> {
    let mut after: Vec<Parameter> = Vec::new();
    loop {
        let predicate = if after.is_empty() {
            ""
        } else {
            "WHERE (location_id,target,observed_at,event_id)>($1,$2,$3,$4)"
        };
        let rows;
        (transaction, rows) = query(
            transaction,
            &format!(
                "SELECT event_id,location_id,target,observed_at,record_json \
                 FROM sqb_warehouse_freshness {predicate} \
                 ORDER BY location_id,target,observed_at,event_id LIMIT 100"
            ),
            after,
        )
        .await?;
        if rows.is_empty() {
            return Ok(transaction);
        }
        after = Vec::new();
        for row in rows {
            let observation: StoredSourceFreshnessObservation = decode_row(row)?;
            after = vec![
                Parameter::Id(observation.location_id),
                Parameter::Text(observation.target.clone()),
                Parameter::Time(observation.observed_at),
                Parameter::Id(observation.event_id),
            ];
            transaction = append_observation(
                transaction,
                (
                    observation.location_id,
                    &observation.target,
                    observation.event_id,
                ),
                &observation.record_json,
            )
            .await?;
        }
    }
}
