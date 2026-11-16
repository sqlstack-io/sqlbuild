//! Native plans of one SQL-test batch, with adapter rendering and timing accounted for.

use crate::sql_test_glue::_helpers::plan::plan_driver;
use crate::sql_test_glue::models::PlanRow;
use crate::sql_test_glue::models::SqlTestPlanOptions;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Native plans of one SQL-test batch, with adapter rendering and timing accounted for.
#[inline]
pub fn planned_rows<'py>(
    (project, adapter): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    tests: &Bound<'py, PyAny>,
    options: &SqlTestPlanOptions<'py>,
) -> PyResult<Vec<PlanRow>> {
    plan_driver::planned_rows((project, adapter), tests, options)
}
