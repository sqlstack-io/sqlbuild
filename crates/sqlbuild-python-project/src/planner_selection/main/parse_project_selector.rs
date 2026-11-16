//! Parse one selector token into Python's `ParsedSelector` or `PathSelector`.

use crate::planner_selection::_helpers::selectors;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyString;

/// Parse one selector token into Python's `ParsedSelector` or `PathSelector`.
#[inline]
pub fn parse_project_selector<'py>(raw: &Bound<'py, PyString>) -> PyResult<Bound<'py, PyAny>> {
    selectors::parse_project_selector(raw)
}
