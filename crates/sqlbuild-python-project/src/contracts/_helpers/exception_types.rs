//! Typed SQL value exceptions.

use pyo3::create_exception;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

create_exception!(
    sqlbuild._native,
    SqlValueValidationError,
    PyValueError,
    "An authored value cannot be represented by the typed SQL value domain."
);

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add(
        "SqlValueValidationError",
        module.py().get_type::<SqlValueValidationError>(),
    )
}
