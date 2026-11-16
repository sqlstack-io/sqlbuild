//! `value.astimezone(UTC)`.

use crate::cursor::constants::DATETIME_MODULE;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::boundary::main::imported::imported;

/// `value.astimezone(UTC)`.
pub fn in_utc<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    value.call_method1(
        "astimezone",
        (imported(value.py(), DATETIME_MODULE, "UTC")?,),
    )
}
