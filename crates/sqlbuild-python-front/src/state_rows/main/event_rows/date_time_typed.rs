//! Whether `value` is a `datetime`.

use crate::cursor::constants::DATETIME_MODULE;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::boundary::main::imported::imported;

/// Whether `value` is a `datetime`.
pub fn date_time_typed(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    value.is_instance(&imported(value.py(), DATETIME_MODULE, "datetime")?)
}
