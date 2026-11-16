//! Preserve public record identity and dataclass behavior on explicit projection.

use crate::planner_scope::_helpers::retained;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Preserve public record identity and dataclass behavior on explicit projection.
#[inline]
pub fn project_scope<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    retained::project_scope(value)
}
