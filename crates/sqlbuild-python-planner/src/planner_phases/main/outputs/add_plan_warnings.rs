//! Add unknown source freshness warnings and, with selection diagnostics, stale-selection warnings.

use crate::planner_phases::_helpers::outputs::output_assembly;
use crate::planner_phases::models::{PlanOutputFields, PlanWarningContext};
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Add unknown source freshness warnings and, with selection diagnostics, stale-selection warnings.
#[inline]
pub fn add_plan_warnings<'py>(
    output: PlanOutputFields<'py>,
    PlanWarningContext {
        scopes,
        warehouse,
        identities,
        stale_warning_changes,
    }: PlanWarningContext<'_, 'py>,
    (source_freshness, policies): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<PlanOutputFields<'py>> {
    output_assembly::add_plan_warnings(
        output,
        PlanWarningContext {
            scopes,
            warehouse,
            identities,
            stale_warning_changes,
        },
        (source_freshness, policies),
    )
}
