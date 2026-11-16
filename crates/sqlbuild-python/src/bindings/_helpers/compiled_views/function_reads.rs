//! A compiled function's attributes the planner reads, from its native view.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::bindings::_helpers::compiled_views::function_views::NativeCompiledFunction;
use sqlbuild_python_base::compile_driver::constants::COMPILED_VIEW_FALLBACKS;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The native view behind a function object; any other object is a Python-built record, counted.
#[inline]
fn native_function<'a>(
    function: &'a Bound<'_, PyAny>,
) -> PyResult<Option<&'a NativeCompiledFunction>> {
    if let Ok(view) = function.cast::<NativeCompiledFunction>() {
        return Ok(Some(view.get()));
    }
    count_stage(function.py(), COMPILED_VIEW_FALLBACKS, 1)?;
    Ok(None)
}

/// The compiled function's `arguments`, as its Python attribute reads it.
pub(crate) fn arguments<'py>(function: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = function.py();
    if let Some(view) = native_function(function)? {
        return view
            .arguments(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(function, "arguments")
}

/// The compiled function's `body_sql`, as its Python attribute reads it.
pub(crate) fn body_sql<'py>(function: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = function.py();
    if let Some(view) = native_function(function)? {
        return view
            .body_sql(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(function, "body_sql")
}

/// The compiled function's `deps`, as its Python attribute reads it.
pub(crate) fn deps<'py>(function: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = function.py();
    if let Some(view) = native_function(function)? {
        return view.deps(py).and_then(|value| value.into_bound_py_any(py));
    }
    native_field(function, "deps")
}

/// The compiled function's `destination`, as its Python attribute reads it.
pub(crate) fn destination<'py>(function: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = function.py();
    if let Some(view) = native_function(function)? {
        return view
            .destination(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(function, "destination")
}

/// The compiled function's `entry_point`, as its Python attribute reads it.
pub(crate) fn entry_point<'py>(function: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = function.py();
    if let Some(view) = native_function(function)? {
        return view
            .entry_point(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(function, "entry_point")
}

/// The compiled function's `fingerprint_destination`, as its Python attribute reads it.
pub(crate) fn fingerprint_destination<'py>(
    function: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = function.py();
    if let Some(view) = native_function(function)? {
        return view
            .fingerprint_destination(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(function, "fingerprint_destination")
}

/// The compiled function's `key`, as its Python attribute reads it.
pub(crate) fn key<'py>(function: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = function.py();
    if let Some(view) = native_function(function)? {
        return view.key(py).and_then(|value| value.into_bound_py_any(py));
    }
    native_field(function, "key")
}

/// The compiled function's `language`, as its Python attribute reads it.
pub(crate) fn language<'py>(function: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = function.py();
    if let Some(view) = native_function(function)? {
        return view
            .language(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(function, "language")
}

/// The compiled function's `name`, as its Python attribute reads it.
pub(crate) fn name<'py>(function: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = function.py();
    if let Some(view) = native_function(function)? {
        return view.name(py).and_then(|value| value.into_bound_py_any(py));
    }
    native_field(function, "name")
}

/// The compiled function's `packages`, as its Python attribute reads it.
pub(crate) fn packages<'py>(function: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = function.py();
    if let Some(view) = native_function(function)? {
        return view
            .packages(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(function, "packages")
}

/// The compiled function's `relative_path`, as its Python attribute reads it.
pub(crate) fn relative_path<'py>(function: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = function.py();
    if let Some(view) = native_function(function)? {
        return view
            .relative_path(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(function, "relative_path")
}

/// The compiled function's `return_columns`, as its Python attribute reads it.
pub(crate) fn return_columns<'py>(function: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = function.py();
    if let Some(view) = native_function(function)? {
        return view
            .return_columns(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(function, "return_columns")
}

/// The compiled function's `returns`, as its Python attribute reads it.
pub(crate) fn returns<'py>(function: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = function.py();
    if let Some(view) = native_function(function)? {
        return view
            .returns(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(function, "returns")
}

/// The compiled function's `runtime_version`, as its Python attribute reads it.
pub(crate) fn runtime_version<'py>(function: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = function.py();
    if let Some(view) = native_function(function)? {
        return view
            .runtime_version(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(function, "runtime_version")
}

/// The compiled function's `source_file_path`, as its Python attribute reads it.
pub(crate) fn source_file_path<'py>(function: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = function.py();
    if let Some(view) = native_function(function)? {
        return view
            .source_file_path(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(function, "source_file_path")
}
