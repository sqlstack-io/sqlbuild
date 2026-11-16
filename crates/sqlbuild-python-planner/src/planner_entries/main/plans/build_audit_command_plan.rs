//! Project selected audits from canonical static state.

use crate::planner_entries::_helpers::plans::command_plans;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Project selected audits from canonical static state.
#[inline]
pub fn build_audit_command_plan<'py>(
    project: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
    scope: &Bound<'py, PyAny>,
    relations: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    command_plans::build_audit_command_plan(project, adapter, scope, relations)
}
