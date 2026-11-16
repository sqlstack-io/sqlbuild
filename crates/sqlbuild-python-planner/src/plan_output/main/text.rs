//! `str(value)`.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

/// `str(value)`.
pub fn text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(value.str()?.to_string())
}
