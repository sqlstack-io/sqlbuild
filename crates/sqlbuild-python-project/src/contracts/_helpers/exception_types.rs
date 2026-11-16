//! Typed SQL value exceptions.

use pyo3::create_exception;
use pyo3::exceptions::PyValueError;

create_exception!(
    sqlbuild._native,
    SqlValueValidationError,
    PyValueError,
    "An authored value cannot be represented by the typed SQL value domain."
);
