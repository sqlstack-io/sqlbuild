//! One page of a large plan section, under the same authorization and diff redaction as the plan.

use sqlbuild_core::plan_display::main::plan_section_page::plan_section_page;
use sqlbuild_core::plan_display::models::{PlanPage, PlanPageSection};
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::platform::constants::PLAN_ROWS_LIMIT;
use crate::platform::main::person_reads::read_person_plan::read_person_plan;
use crate::platform::models::{LocationWorkOutcome, PlanRowSection, PlanRows, PlanRowsQuery};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// The page `query` names of the requester's plan, found by name; `None` until the plan exists and is planned.
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
            LocationWorkOutcome::Plan { document, .. } => Some(document),
            _ => None,
        });
    let Some(document) = document else {
        return Ok(None);
    };
    let limit: usize = query.limit.unwrap_or(PLAN_ROWS_LIMIT).min(PLAN_ROWS_LIMIT);
    let page: PlanPage = plan_section_page(
        &document,
        page_section(query.section),
        ((query.offset.unwrap_or(0), limit), query.name.as_deref()),
    );
    Ok(Some(PlanRows {
        total: page.total,
        section_total: page.section_total,
        rows: page.rows,
        runs: page.runs,
        warnings: page.warnings,
        cursor_window_starts: page
            .cursor_window_starts
            .into_iter()
            .filter_map(|(model, start)| Some((model, start.as_str()?.to_owned())))
            .collect(),
    }))
}

/// The plan display's section for the API's.
fn page_section(section: PlanRowSection) -> PlanPageSection {
    match section {
        PlanRowSection::FullRefresh => PlanPageSection::FullRefresh,
        PlanRowSection::QueryChanged => PlanPageSection::QueryChanged,
        PlanRowSection::FunctionChanged => PlanPageSection::FunctionChanged,
        PlanRowSection::ConfigChanged => PlanPageSection::ConfigChanged,
        PlanRowSection::SchemaChanged => PlanPageSection::SchemaChanged,
        PlanRowSection::FirstRun => PlanPageSection::FirstRun,
        PlanRowSection::Renamed => PlanPageSection::Renamed,
        PlanRowSection::RoutineModels => PlanPageSection::RoutineModels,
        PlanRowSection::Migrations => PlanPageSection::Migrations,
        PlanRowSection::ColumnMigrations => PlanPageSection::ColumnMigrations,
        PlanRowSection::TableTypeConversions => PlanPageSection::TableTypeConversions,
        PlanRowSection::Retention => PlanPageSection::Retention,
        PlanRowSection::LoaderReloads => PlanPageSection::LoaderReloads,
        PlanRowSection::LoaderLoads => PlanPageSection::LoaderLoads,
        PlanRowSection::SourceReloads => PlanPageSection::SourceReloads,
        PlanRowSection::SourceLoads => PlanPageSection::SourceLoads,
        PlanRowSection::ChangedFunctions => PlanPageSection::ChangedFunctions,
        PlanRowSection::Functions => PlanPageSection::Functions,
        PlanRowSection::FirstRunSeeds => PlanPageSection::FirstRunSeeds,
        PlanRowSection::ChangedSeeds => PlanPageSection::ChangedSeeds,
        PlanRowSection::UnchangedSeeds => PlanPageSection::UnchangedSeeds,
        PlanRowSection::PythonNodes => PlanPageSection::PythonNodes,
        PlanRowSection::HeldBack => PlanPageSection::HeldBack,
        PlanRowSection::Unchanged => PlanPageSection::Unchanged,
        PlanRowSection::Warnings => PlanPageSection::Warnings,
    }
}
