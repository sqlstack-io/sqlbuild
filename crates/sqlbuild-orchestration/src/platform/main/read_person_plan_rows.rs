//! One page of a large plan section, under the same authorization and diff redaction as the plan.

use sqlbuild_core::plan_display::main::plan_section_page::plan_section_page;
use sqlbuild_core::plan_display::models::{PlanPage, PlanPageSection};
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::platform::constants::PLAN_ROWS_LIMIT;
use crate::platform::main::read_person_plan::read_person_plan;
use crate::platform::models::{LocationWorkOutcome, PlanRowSection, PlanRows, PlanRowsQuery};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// The page `query` names of the requester's plan; `None` until the plan exists and is planned.
pub async fn read_person_plan_rows(
    store: &ControlStore,
    reader: (&AuditContext, Uuid),
    plan: (Uuid, &str, Uuid),
    query: PlanRowsQuery,
) -> Result<Option<PlanRows>, ControlError> {
    let document = read_person_plan(store, reader, plan)
        .await?
        .and_then(|record| record.result_json)
        .and_then(|result| match result.outcome {
            LocationWorkOutcome::Plan { document } => Some(document),
            _ => None,
        });
    let Some(document) = document else {
        return Ok(None);
    };
    let section: PlanPageSection = match query.section {
        PlanRowSection::RoutineModels => PlanPageSection::RoutineModels,
        PlanRowSection::HeldBack => PlanPageSection::HeldBack,
        PlanRowSection::Unchanged => PlanPageSection::Unchanged,
    };
    let limit: usize = query.limit.unwrap_or(PLAN_ROWS_LIMIT).min(PLAN_ROWS_LIMIT);
    let PlanPage { total, rows, runs } =
        plan_section_page(&document, section, (query.offset.unwrap_or(0), limit));
    Ok(Some(PlanRows { total, rows, runs }))
}
