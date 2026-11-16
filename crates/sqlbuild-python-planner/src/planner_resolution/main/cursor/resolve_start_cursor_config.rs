//! Resolve automatic-start policy with model config taking precedence.

use crate::planner_resolution::_helpers::cursor::cursor_policies;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Resolve automatic-start policy with model config taking precedence.
#[inline]
pub fn resolve_start_cursor_config<'py>(
    model: &Bound<'py, PyAny>,
    project_config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    cursor_policies::resolve_start_cursor_config(model, project_config)
}
