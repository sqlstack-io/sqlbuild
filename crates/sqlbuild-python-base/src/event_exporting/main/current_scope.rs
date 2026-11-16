//! The active command-owned exporter scope, or `None`.

use crate::event_exporting::_helpers::scope;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The active command-owned exporter scope, or `None`.
#[inline]
pub fn current_scope(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    scope::current_scope(py)
}
