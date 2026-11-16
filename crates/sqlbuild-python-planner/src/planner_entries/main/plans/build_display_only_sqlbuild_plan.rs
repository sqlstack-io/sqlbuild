//! Build a display-only plan when warehouse planning cannot fully resolve.

use crate::planner_entries::_helpers::plans::display_plan;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Build a display-only plan when warehouse planning cannot fully resolve.
#[inline]
pub fn build_display_only_sqlbuild_plan<'py>(
    project: &Bound<'py, PyAny>,
    selected_model_names: &Bound<'py, PyAny>,
    full_refresh: bool,
) -> PyResult<Bound<'py, PyAny>> {
    display_plan::build_display_only_sqlbuild_plan(project, selected_model_names, full_refresh)
}
