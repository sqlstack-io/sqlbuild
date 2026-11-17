//! The compiled seed's `external`, as its Python attribute reads it.

use crate::resource_views::_helpers::seed_reads::native_seed;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The compiled seed's `external`, as its Python attribute reads it.
pub fn external<'py>(seed: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = seed.py();
    if let Some(view) = native_seed(seed)? {
        return view.is_external().into_bound_py_any(py);
    }
    native_field(seed, "external")
}
