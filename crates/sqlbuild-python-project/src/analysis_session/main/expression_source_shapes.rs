//! One shape per expression, cached on `profile`'s binding catalog; a failure raises.

use crate::analysis_session::_helpers::stage_functions;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyTuple;

/// One shape per expression, cached on `profile`'s binding catalog; a failure raises.
#[inline]
pub fn expression_source_shapes<'py>(
    expressions: &Bound<'py, PyAny>,
    profile: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    stage_functions::expression_source_shapes(expressions, profile)
}
