//! Resolve automatic-start policy with model config taking precedence.

use crate::planner_resolution::_helpers::cursor::cursor_policies::start_cursor_config;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Resolve automatic-start policy with model config taking precedence.
pub fn resolve_start_cursor_config<'py>(
    model: &Bound<'py, PyAny>,
    project_config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    start_cursor_config(model, project_config)
}
