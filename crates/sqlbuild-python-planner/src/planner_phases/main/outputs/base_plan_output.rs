//! The plan output's base fields from the planned entries, relations and source freshness.

use crate::planner_phases::_helpers::outputs::output_assembly;
use crate::planner_phases::models::{PlanOutputContext, PlanOutputFields, PlannedModelEntries};
use pyo3::prelude::{Bound, PyAny, PyResult};

/// The plan output's base fields from the planned entries, relations and source freshness.
#[inline]
pub fn base_plan_output<'py>(
    PlanOutputContext {
        runtime,
        warehouse,
        identities,
        overrides,
        audits_not_run,
    }: PlanOutputContext<'_, 'py>,
    PlannedModelEntries {
        execution_scope,
        reconciliation,
        entries,
        source_freshness,
    }: PlannedModelEntries<'_, 'py>,
    planned_sql_tests: &Bound<'py, PyAny>,
) -> PyResult<PlanOutputFields<'py>> {
    output_assembly::base_plan_output(
        PlanOutputContext {
            runtime,
            warehouse,
            identities,
            overrides,
            audits_not_run,
        },
        PlannedModelEntries {
            execution_scope,
            reconciliation,
            entries,
            source_freshness,
        },
        planned_sql_tests,
    )
}
