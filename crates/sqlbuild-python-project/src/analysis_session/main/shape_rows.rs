//! `{relation: {column: value}}` as ordered rows, each value as `str(value)`.

use crate::analysis_session::_helpers::profile_inputs;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::assembly::analysis_session::types::Shapes;

/// `{relation: {column: value}}` as ordered rows, each value as `str(value)`.
#[inline]
pub fn shape_rows(shapes: &Bound<'_, PyAny>) -> PyResult<Shapes> {
    profile_inputs::shape_rows(shapes)
}
