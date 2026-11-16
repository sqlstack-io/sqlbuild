//! The compiled function's `return_columns`, as its Python attribute reads it.

use crate::resource_views::_helpers::function_reads::native_function;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The compiled function's `return_columns`, as its Python attribute reads it.
pub fn return_columns<'py>(function: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = function.py();
    if let Some(view) = native_function(function)? {
        return view
            .return_columns(py)
            .and_then(|value| value.into_bound_py_any(py));
    }
    native_field(function, "return_columns")
}
