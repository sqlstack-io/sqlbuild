//! `datetime.fromisoformat(text.replace("Z", "+00:00"))`, or `None` on `ValueError`.

use crate::cursor::constants::DATETIME_MODULE;
use crate::execution_history::_helpers::conversion::UTC_OFFSET;
use crate::execution_history::_helpers::conversion::UTC_SUFFIX;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_python_base::boundary::main::imported::imported;

/// `datetime.fromisoformat(text.replace("Z", "+00:00"))`, or `None` on `ValueError`.
pub fn parsed_iso<'py>(py: Python<'py>, text: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
    let normalized: String = text.replace(UTC_SUFFIX, UTC_OFFSET);
    match imported(py, DATETIME_MODULE, "datetime")?.call_method1("fromisoformat", (normalized,)) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.is_instance_of::<PyValueError>(py) => Ok(None),
        Err(error) => Err(error),
    }
}
