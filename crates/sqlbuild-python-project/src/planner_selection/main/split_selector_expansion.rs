//! Split `+selector+` marker syntax from the selector core without resolving the core.

use crate::planner_selection::_helpers::model_selectors;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyString;

/// Split `+selector+` marker syntax from the selector core without resolving the core.
#[inline]
pub fn split_selector_expansion<'py>(raw: &Bound<'py, PyString>) -> PyResult<Bound<'py, PyAny>> {
    model_selectors::split_selector_expansion(raw)
}
