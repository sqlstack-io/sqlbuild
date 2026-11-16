//! Render `<purpose>, set this in <file>:` followed by the exact TOML section and key.

use crate::discovery::_helpers::failures::guidance;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;

/// Render `<purpose>, set this in <file>:` followed by the exact TOML section and key.
#[inline]
pub fn setting_help<'py>(py: Python<'py>, options: Option<Bound<'py, PyDict>>) -> PyResult<String> {
    guidance::setting_help(py, options)
}
