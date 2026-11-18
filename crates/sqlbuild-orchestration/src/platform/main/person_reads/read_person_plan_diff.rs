//! One planned model's diff text, under the plan's authorization, and only with view.sql.

use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::platform::_helpers::plans::plan_access::read_person_plan_with_sql;
use crate::platform::_helpers::plans::plan_diffs::read_plan_diff;
use crate::platform::models::{LocationWorkOutcome, PlanDiffRead};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit};
use crate::store::models::{ControlStore, TransactionMode};

/// The model's diff the plan stored beside its document; `None` without one or without view.sql everywhere.
pub async fn read_person_plan_diff(
    store: &ControlStore,
    reader: (&AuditContext, Uuid),
    (plan, model): ((Uuid, &str, Uuid), &str),
) -> Result<Option<PlanDiffRead>, ControlError> {
    let Some((record, true)) = read_person_plan_with_sql(store, reader, plan).await? else {
        return Ok(None);
    };
    if !matches!(
        record.result_json.map(|result| result.outcome),
        Some(LocationWorkOutcome::Plan { .. })
    ) {
        return Ok(None);
    }
    let (transaction, diff) =
        read_plan_diff(begin(store, TransactionMode::Read).await?, (plan.2, model)).await?;
    commit(transaction).await?;
    Ok(diff.map(|diff| PlanDiffRead {
        model: model.to_owned(),
        diff,
    }))
}
