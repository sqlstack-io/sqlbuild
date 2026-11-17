//! `datetime.UTC`.

use crate::cursor::constants::DATETIME_MODULE;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::boundary::main::imported::imported;

/// `datetime.UTC`.
pub fn utc(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    imported(py, DATETIME_MODULE, "UTC")
}
