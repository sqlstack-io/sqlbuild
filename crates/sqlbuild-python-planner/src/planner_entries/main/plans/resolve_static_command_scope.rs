//! Resolve command selection through the canonical build selector phase.

use crate::planner_entries::_helpers::entries::values::{boolean, planner_model};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::planner_scope::main::resolve_planner_scopes::resolve_planner_scopes;

/// Resolve command selection through the canonical build selector phase.
#[pyfunction]
#[pyo3(signature = (*, project, selection, auto_load_sources = false))]
pub fn resolve_static_command_scope<'py>(
    project: &Bound<'py, PyAny>,
    selection: &Bound<'py, PyAny>,
    auto_load_sources: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py = project.py();
    resolve_planner_scopes(
        project,
        selection,
        &planner_model(
            py,
            RecordId::PlannerPolicies,
            &[("auto_load_sources", boolean(py, auto_load_sources)?)],
        )?,
    )?
    .getattr("selected_scope")
}
