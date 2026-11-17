//! A new view of the process environment as compile text reads it.

use crate::model_config::_helpers::model_nodes::unicode_environment;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// A new view of the process environment as compile text reads it.
#[inline]
pub fn unicode_environment(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    unicode_environment::unicode_environment(py)
}
