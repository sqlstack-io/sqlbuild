//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::bindings::_helpers::dbt_interop::graph::DbtCombinedGraph;
use crate::bindings::_helpers::dbt_interop::reference_resolver::DbtCompileReferenceResolver;
use crate::bindings::_helpers::dbt_interop::runner::DbtRunner;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub(crate) const GETTERS: &[GetterEntry] = &[
    GetterEntry {
        class: class_of::<DbtCombinedGraph>,
        read: read_dbt_combined_graph,
    },
    GetterEntry {
        class: class_of::<DbtCompileReferenceResolver>,
        read: read_dbt_compile_reference_resolver,
    },
    GetterEntry {
        class: class_of::<DbtRunner>,
        read: read_dbt_runner,
    },
];

fn read_dbt_combined_graph<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<DbtCombinedGraph>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "nodes" => this.nodes(py).and_then(|value| value.into_bound_py_any(py)),
        "upstream_deps" => this
            .upstream_deps(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "downstream_deps" => this
            .downstream_deps(py)
            .and_then(|value| value.into_bound_py_any(py)),
        _ => return None,
    })
}

fn read_dbt_compile_reference_resolver<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<DbtCompileReferenceResolver>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "_dbt_manifest" => this.dbt_manifest(py).into_bound_py_any(py),
        _ => return None,
    })
}

fn read_dbt_runner<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<DbtRunner>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "dbt_executable" => (&this.dbt_executable).into_bound_py_any(py),
        "invoker" => (&this.invoker).into_bound_py_any(py),
        _ => return None,
    })
}
