//! One string-valued model configuration entry.

use crate::planner_resolution::_helpers::cursor::cursor_ranges;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// One string-valued model configuration entry.
#[inline]
pub fn config_str<'py>(model: &Bound<'py, PyAny>, key: &str) -> PyResult<Bound<'py, PyAny>> {
    cursor_ranges::config_str(model, key)
}
