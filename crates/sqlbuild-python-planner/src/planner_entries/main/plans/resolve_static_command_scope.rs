//! Resolve command selection through the canonical build selector phase.

use crate::planner_entries::_helpers::plans::command_plans;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Resolve command selection through the canonical build selector phase.
#[inline]
pub fn resolve_static_command_scope<'py>(
    project: &Bound<'py, PyAny>,
    selection: &Bound<'py, PyAny>,
    auto_load_sources: bool,
) -> PyResult<Bound<'py, PyAny>> {
    command_plans::resolve_static_command_scope(project, selection, auto_load_sources)
}
