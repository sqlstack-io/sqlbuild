//! Closed shapes by relation name; absence means open, never an empty physical table.

use crate::semantic_shapes::_helpers::shapes;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyDict;

/// Closed shapes by relation name; absence means open, never an empty physical table.
#[inline]
pub fn project_shapes<'py>(
    project: &Bound<'py, PyAny>,
    profile: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyDict>> {
    shapes::project_shapes(project, profile)
}
