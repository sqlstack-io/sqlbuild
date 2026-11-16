//! Register expected errors.

use crate::boundary::_helpers::expected_errors;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// register_expected_errors.
#[inline]
pub fn register_expected_errors(module: &Bound<'_, PyModule>) -> PyResult<()> {
    expected_errors::register(module)
}
