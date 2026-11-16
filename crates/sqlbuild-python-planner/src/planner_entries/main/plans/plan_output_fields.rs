//! The plan output's fields from planned entries and resolved relation inputs.

use crate::planner_entries::_helpers::plans::plan_output;
use crate::planner_entries::models::PlanOutputInputs;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// The plan output's fields from planned entries and resolved relation inputs.
#[inline]
pub fn plan_output_fields<'py>(
    inputs: PlanOutputInputs<'py>,
) -> PyResult<Vec<(&'static str, Bound<'py, PyAny>)>> {
    plan_output::plan_output_fields(inputs)
}
