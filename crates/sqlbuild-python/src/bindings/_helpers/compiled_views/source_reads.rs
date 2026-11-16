//! A compiled source's attributes the planner reads, from its native view.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::bindings::_helpers::compiled_views::source_views::NativeCompiledSource;
use sqlbuild_python_base::compile_driver::constants::COMPILED_VIEW_FALLBACKS;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The native view behind a source object; any other object is a Python-built record, counted.
#[inline]
fn native_source<'a>(source: &'a Bound<'_, PyAny>) -> PyResult<Option<&'a NativeCompiledSource>> {
    if let Ok(view) = source.cast::<NativeCompiledSource>() {
        return Ok(Some(view.get()));
    }
    count_stage(source.py(), COMPILED_VIEW_FALLBACKS, 1)?;
    Ok(None)
}

/// The compiled source's `key`, as its Python attribute reads it.
pub(crate) fn key<'py>(source: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = source.py();
    if let Some(view) = native_source(source)? {
        return view.key(py).and_then(|value| value.into_bound_py_any(py));
    }
    native_field(source, "key")
}

/// The compiled source's `name`, as its Python attribute reads it.
pub(crate) fn name<'py>(source: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = source.py();
    if let Some(view) = native_source(source)? {
        return view.name(py).and_then(|value| value.into_bound_py_any(py));
    }
    native_field(source, "name")
}

/// The compiled source's `source_entry`, as its Python attribute reads it.
pub(crate) fn source_entry<'py>(source: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = source.py();
    if let Some(view) = native_source(source)? {
        return view
            .source_entry(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(source, "source_entry")
}

/// The compiled source's `source_file`, as its Python attribute reads it.
pub(crate) fn source_file<'py>(source: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = source.py();
    if let Some(view) = native_source(source)? {
        return view
            .source_file(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(source, "source_file")
}
