//! A compiled seed's attributes the planner reads, from its native view.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::bindings::_helpers::compiled_views::seed_views::NativeCompiledSeed;
use sqlbuild_python_base::compile_driver::constants::COMPILED_VIEW_FALLBACKS;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The native view behind a seed object; any other object is a Python-built record, counted.
#[inline]
fn native_seed<'a>(seed: &'a Bound<'_, PyAny>) -> PyResult<Option<&'a NativeCompiledSeed>> {
    if let Ok(view) = seed.cast::<NativeCompiledSeed>() {
        return Ok(Some(view.get()));
    }
    count_stage(seed.py(), COMPILED_VIEW_FALLBACKS, 1)?;
    Ok(None)
}

/// The compiled seed's `destination`, as its Python attribute reads it.
pub(crate) fn destination<'py>(seed: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = seed.py();
    if let Some(view) = native_seed(seed)? {
        return view
            .destination(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(seed, "destination")
}

/// The compiled seed's `external`, as its Python attribute reads it.
pub(crate) fn external<'py>(seed: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = seed.py();
    if let Some(view) = native_seed(seed)? {
        return view.is_external().into_bound_py_any(py);
    }
    native_field(seed, "external")
}

/// The compiled seed's `key`, as its Python attribute reads it.
pub(crate) fn key<'py>(seed: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = seed.py();
    if let Some(view) = native_seed(seed)? {
        return view.key(py).and_then(|value| value.into_bound_py_any(py));
    }
    native_field(seed, "key")
}

/// The compiled seed's `name`, as its Python attribute reads it.
pub(crate) fn name<'py>(seed: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = seed.py();
    if let Some(view) = native_seed(seed)? {
        return view.name(py).and_then(|value| value.into_bound_py_any(py));
    }
    native_field(seed, "name")
}

/// The compiled seed's `schema_entry`, as its Python attribute reads it.
pub(crate) fn schema_entry<'py>(seed: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = seed.py();
    if let Some(view) = native_seed(seed)? {
        return view
            .schema_entry(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(seed, "schema_entry")
}

/// The compiled seed's `seed_file`, as its Python attribute reads it.
pub(crate) fn seed_file<'py>(seed: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = seed.py();
    if let Some(view) = native_seed(seed)? {
        return view
            .seed_file(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(seed, "seed_file")
}
