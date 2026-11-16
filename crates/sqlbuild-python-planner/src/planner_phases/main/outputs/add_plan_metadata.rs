//! Add the direct-mode source freshness, remaining stale models and selection diagnostics metadata.

use crate::planner_phases::_helpers::outputs::output_assembly;
use crate::planner_phases::models::PlanOutputFields;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Add the direct-mode source freshness, remaining stale models and selection diagnostics metadata.
#[inline]
pub fn add_plan_metadata<'py>(
    output: PlanOutputFields<'py>,
    source_freshness: &Bound<'py, PyAny>,
    policies: &Bound<'py, PyAny>,
) -> PyResult<PlanOutputFields<'py>> {
    output_assembly::add_plan_metadata(output, source_freshness, policies)
}
