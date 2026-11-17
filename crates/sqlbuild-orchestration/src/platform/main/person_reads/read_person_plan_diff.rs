//! One planned model's diff text, under the plan's authorization, and only with view.sql.

use sqlbuild_core::plan_display::main::plan_model_diff::plan_model_diff;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::platform::_helpers::plans::plan_access::read_person_plan_with_sql;
use crate::platform::_helpers::plans::plan_diffs::read_plan_diff;
use crate::platform::models::{LocationWorkOutcome, PlanDiffRead, PlanModelDiff};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// The model's diff from the plan document or stored apart; `None` without one or without view.sql everywhere.
pub async fn read_person_plan_diff(
    store: &ControlStore,
    reader: (&AuditContext, Uuid),
    (plan, model): ((Uuid, &str, Uuid), &str),
) -> Result<Option<PlanDiffRead>, ControlError> {
    let Some((record, true)) = read_person_plan_with_sql(store, reader, plan).await? else {
        return Ok(None);
    };
    let Some(LocationWorkOutcome::Plan { document, .. }) =
        record.result_json.map(|result| result.outcome)
    else {
        return Ok(None);
    };
    let inline = plan_model_diff(&document, model).map(|diff| PlanModelDiff {
        query_diff: diff.query_diff,
        config_diff: diff.config_diff,
    });
    let diff = match inline {
        Some(diff) => Some(diff),
        None => {
            let (transaction, stored) =
                read_plan_diff(begin(store, TransactionMode::Read).await?, (plan.2, model)).await?;
            commit(transaction).await?;
            stored
        }
    };
    Ok(diff.map(|diff| PlanDiffRead {
        model: model.to_owned(),
        diff,
    }))
}
