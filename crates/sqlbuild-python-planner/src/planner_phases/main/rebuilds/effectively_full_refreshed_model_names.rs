//! Names of models effectively full refreshed for this invocation.

use crate::planner_phases::_helpers::rebuilds::full_refresh;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Names of models effectively full refreshed for this invocation.
#[inline]
pub fn effectively_full_refreshed_model_names<'py>(
    project: &Bound<'py, PyAny>,
    cli_full_refresh: bool,
) -> PyResult<Bound<'py, PyAny>> {
    full_refresh::effectively_full_refreshed_model_names(project, cli_full_refresh)
}
