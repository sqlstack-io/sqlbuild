//! Resolve the selected, stale-warning and inspection scopes of one plan.

use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::planner_phases::models::PlannerScopeResolution;
use crate::bindings::_helpers::planner_scope::retained::scope_data;
use crate::bindings::_helpers::planner_scope::scope::build_planner_scope;
use crate::bindings::_helpers::records::fields::field as native_field;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyTuple;
use sqlbuild_analysis::graph::main::reachable_keys::reachable_keys;

#[pyfunction]
#[pyo3(signature = (*, project, selection, policies))]
pub(crate) fn resolve_planner_scopes<'py>(
    project: &Bound<'py, PyAny>,
    selection: &Bound<'py, PyAny>,
    policies: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = project.py();
    let auto_load = native_field(policies, "auto_load_sources")?;
    let selected = build_planner_scope(
        py,
        Some(keywords(
            py,
            &[
                ("project", project.clone()),
                ("select", native_field(selection, "select")?),
                ("exclude", native_field(selection, "exclude")?),
                ("auto_load_sources", auto_load.clone()),
                ("selected_keys", native_field(selection, "selected_keys")?),
                (
                    "python_read_source_names",
                    native_field(selection, "python_read_source_names")?,
                ),
                (
                    "sql_test_selection",
                    native_field(selection, "sql_test_selection")?,
                ),
            ],
        )?),
    )?;
    let empty = PyTuple::empty(py).into_any();
    let full = build_planner_scope(
        py,
        Some(keywords(
            py,
            &[
                ("project", project.clone()),
                ("select", empty.clone()),
                ("exclude", empty),
                ("auto_load_sources", auto_load),
            ],
        )?),
    )?;
    let selected_data = scope_data(&selected)?;
    let full_data = scope_data(&full)?;
    let stale_keys = reachable_keys(
        &selected_data.selected.iter().cloned().collect::<Vec<_>>(),
        &full_data.upstream,
    );
    let mut stale = full_data.with_selected(py, selected_data.selected.clone());
    stale.names.retain(|(_, key)| stale_keys.contains(key));
    stale
        .models
        .retain(|name, _| stale_keys.contains(&("model".to_owned(), name.clone())));
    stale.order.retain(|key| stale_keys.contains(key));
    stale.user_selected = selected_data.user_selected.clone();
    let stale_scope = stale.object(py)?;
    Ok(Bound::new(
        py,
        PlannerScopeResolution {
            selected_scope: selected.clone().unbind(),
            stale_warning_scope: stale_scope.unbind(),
            inspection_scope: selected.unbind(),
        },
    )?
    .into_any())
}
