//! Model diff text a plan keeps apart from its document, written with the plan and read one model at a time.

use std::collections::BTreeMap;

use serde_json::json;
use uuid::Uuid;

use crate::platform::constants::{PLAN_DIFF_BYTES, PLAN_DIFF_INSERT_ROWS};
use crate::platform::models::{LocationWorkOutcome, LocationWorkResult, PlanModelDiff};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// Store each model's diff of the plan, a batch of rows per statement.
pub(crate) async fn store_plan_diffs(
    mut transaction: ControlTransaction,
    request_id: Uuid,
    diffs: &BTreeMap<String, PlanModelDiff>,
) -> Result<ControlTransaction, ControlError> {
    let rows: Vec<(&String, &PlanModelDiff)> = diffs.iter().collect();
    for batch in rows.chunks(PLAN_DIFF_INSERT_ROWS) {
        let mut values: Vec<String> = Vec::new();
        let mut parameters: Vec<Parameter> = vec![Parameter::Id(request_id)];
        for (model, diff) in batch {
            let at = parameters.len();
            values.push(format!("($1, ${}, ${})", at + 1, at + 2));
            parameters.push(Parameter::Text((*model).clone()));
            parameters.push(Parameter::Json(json!(diff)));
        }
        (transaction, _) = query(
            transaction,
            &format!(
                "INSERT INTO sqb_plan_diffs (request_id, model, diff_json) VALUES {}",
                values.join(", ")
            ),
            parameters,
        )
        .await?;
    }
    Ok(transaction)
}

/// The model's diff the plan stored apart from its document, if it did.
pub(crate) async fn read_plan_diff(
    transaction: ControlTransaction,
    (request_id, model): (Uuid, &str),
) -> Result<(ControlTransaction, Option<PlanModelDiff>), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT diff_json FROM sqb_plan_diffs WHERE request_id=$1 AND model=$2",
        vec![Parameter::Id(request_id), Parameter::Text(model.to_owned())],
    )
    .await?;
    let diff = rows
        .into_iter()
        .next()
        .and_then(|row| row.get("diff_json").cloned())
        .map(serde_json::from_value::<PlanModelDiff>)
        .transpose()
        .map_err(|error| ControlError::Schema(format!("plan diff: {error}")))?;
    Ok((transaction, diff))
}

/// The result as stored, without the diffs a plan carries beside its document, and those diffs.
pub(crate) fn split_plan_diffs(
    mut result: LocationWorkResult,
) -> (LocationWorkResult, BTreeMap<String, PlanModelDiff>) {
    let diffs = match &mut result.outcome {
        LocationWorkOutcome::Plan { diffs, .. } => std::mem::take(diffs),
        _ => BTreeMap::new(),
    };
    (result, diffs)
}

/// Every diff names its model and has text, and all of them fit the stored bound.
pub(crate) fn valid_plan_diffs(diffs: &BTreeMap<String, PlanModelDiff>) -> bool {
    diffs.iter().all(|(model, diff)| {
        !model.trim().is_empty() && (diff.query_diff.is_some() || diff.config_diff.is_some())
    }) && serde_json::to_vec(diffs).is_ok_and(|bytes| bytes.len() <= PLAN_DIFF_BYTES)
}
