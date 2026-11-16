//! A model config value as text, `None` when absent or not text.

use crate::planner_phases::_helpers::objects;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A model config value as text, `None` when absent or not text.
#[inline]
pub fn config_str(model: &Bound<'_, PyAny>, key: &str) -> PyResult<Option<String>> {
    objects::config_str(model, key)
}
