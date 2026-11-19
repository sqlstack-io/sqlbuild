//! A plan request's state with its summary, which the plan page polls instead of the whole document.

use serde_json::Value;
use sqlbuild_core::plan_display::main::plan_summary::plan_summary;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::platform::constants::PLAN_TIMINGS_KEY;
use crate::platform::main::person_reads::read_person_plan::read_person_plan;
use crate::platform::models::{LocationWorkOutcome, PlanSummaryRead, PlanTimings};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// The requester's plan as `read_person_plan` authorizes it, with a plan document as its summary.
pub async fn read_person_plan_summary(
    store: &ControlStore,
    reader: (&AuditContext, Uuid),
    plan: (Uuid, &str, Uuid),
) -> Result<Option<PlanSummaryRead>, ControlError> {
    let Some(record) = read_person_plan(store, reader, plan).await? else {
        return Ok(None);
    };
    let timings: Option<PlanTimings> = match &record.result_json {
        Some(result) => match &result.outcome {
            LocationWorkOutcome::Plan { document, .. } => plan_timings(document)?,
            _ => None,
        },
        None => None,
    };
    let outcome: Option<LocationWorkOutcome> =
        record.result_json.map(|result| match result.outcome {
            LocationWorkOutcome::Plan { document, diffs } => LocationWorkOutcome::Plan {
                document: plan_summary(&document),
                diffs,
            },
            outcome => outcome,
        });
    Ok(Some(PlanSummaryRead {
        expires_at: record.expires_at,
        outcome,
        timings,
    }))
}

/// The document's planning timings; a failed plan's report has none.
fn plan_timings(document: &Value) -> Result<Option<PlanTimings>, ControlError> {
    let Some(timings) = document.get(PLAN_TIMINGS_KEY) else {
        return Ok(None);
    };
    serde_json::from_value(timings.clone())
        .map(Some)
        .map_err(|error| ControlError::Invalid(format!("plan timings: {error}")))
}
