//! Selected models and seeds, and selected sources with loaders, whose relations this invocation produces.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyFrozenSet;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_project::planner_scope::main::scope_data::scope_data;

/// Selected models and seeds, and selected sources with loaders, whose relations this invocation produces.
#[pyfunction]
#[pyo3(signature = (*, scope, source_map))]
pub fn runtime_cursor_producer_names<'py>(
    scope: &Bound<'py, PyAny>,
    source_map: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let py: Python<'py> = scope.py();
    let data = scope_data(scope)?;
    let mut names: Vec<&str> = Vec::new();
    for (kind, name) in &data.selected {
        match kind.as_str() {
            "model" | "seed" => names.push(name),
            "source" => {
                let source: Bound<'py, PyAny> = mapping_get(source_map, name)?;
                if !source.is_none() && !native_field(&source, "loader")?.is_none() {
                    names.push(name);
                }
            }
            _ => {}
        }
    }
    PyFrozenSet::new(py, names)
}
