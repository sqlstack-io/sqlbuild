//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::project_assembly::models::NativeModelFacts;
use crate::project_assembly::models::NativeProjectFacts;
use crate::project_assembly::models::NativeProjectResources;
use crate::project_assembly::models::NativeResourceFailure;

pub(crate) fn read_native_model_facts<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<NativeModelFacts>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "deps" => (&this.deps).into_bound_py_any(py),
        "dynamic_contract" => (&this.dynamic_contract).into_bound_py_any(py),
        "syntax_valid" => (&this.syntax_valid).into_bound_py_any(py),
        _ => return None,
    })
}

pub(crate) fn read_native_project_facts<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<NativeProjectFacts>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "models" => (&this.models).into_bound_py_any(py),
        "resources" => (&this.resources).into_bound_py_any(py),
        _ => return None,
    })
}

pub(crate) fn read_native_project_resources<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<NativeProjectResources>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "model_deps" => (&this.model_deps).into_bound_py_any(py),
        "model_syntax_valid" => (&this.model_syntax_valid).into_bound_py_any(py),
        "source_namespaces" => (&this.source_namespaces).into_bound_py_any(py),
        "seed_namespaces" => (&this.seed_namespaces).into_bound_py_any(py),
        "function_deps" => (&this.function_deps).into_bound_py_any(py),
        "audit_scope_deps" => (&this.audit_scope_deps).into_bound_py_any(py),
        "failure" => (&this.failure).into_bound_py_any(py),
        _ => return None,
    })
}

pub(crate) fn read_native_resource_failure<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<NativeResourceFailure>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "slot" => (&this.slot).into_bound_py_any(py),
        "index" => (&this.index).into_bound_py_any(py),
        "error" => (&this.error).into_bound_py_any(py),
        _ => return None,
    })
}
