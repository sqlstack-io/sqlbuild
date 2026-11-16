//! Resolve authored cursor input fields into explicit effective roles.

use crate::compile_attachment::_helpers::cursor_roles;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Resolve authored cursor input fields into explicit effective roles.
#[inline]
pub fn resolve_cursor_input_roles<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    cursor_roles::resolve_cursor_input_roles(model)
}
