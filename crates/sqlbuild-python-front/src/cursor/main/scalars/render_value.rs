//! A scalar in the existing cursor output format.

use crate::cursor::models::CursorSession;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use sqlbuild_cursor::algebra::main::render::render;
use sqlbuild_cursor::algebra::models::Scalar;

/// A scalar in the existing cursor output format.
#[pyfunction(name = "cursor_render")]
#[pyo3(signature = (*, value))]
pub fn render_value(value: &Bound<'_, PyAny>) -> PyResult<String> {
    let session: CursorSession<'_> = CursorSession::new(value.py())?;
    let scalar: Scalar = session.scalar(value)?;
    session.checked(render(&scalar, &session))
}
