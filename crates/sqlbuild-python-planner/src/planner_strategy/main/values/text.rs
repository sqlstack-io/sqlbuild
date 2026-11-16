//! `str(value)` of a Python string or string enum.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

/// `str(value)` of a Python string or string enum.
pub fn text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    value.extract()
}
