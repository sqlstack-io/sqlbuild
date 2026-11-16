//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::graph::models::CompiledProjectGraph;

pub(crate) fn read_compiled_project_graph<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<CompiledProjectGraph>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "project" => this.project_attribute(py).into_bound_py_any(py),
        "native" => this.native_attribute(py).into_bound_py_any(py),
        "lineage_views" => this
            .lineage_views(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "upstream_deps" => this
            .upstream_deps(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "downstream_deps" => this
            .downstream_deps(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "tag_index" => this
            .tag_index(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "path_index" => this
            .path_index(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "all_keys" => this
            .all_keys(py)
            .and_then(|value| value.into_bound_py_any(py)),
        _ => return None,
    })
}
