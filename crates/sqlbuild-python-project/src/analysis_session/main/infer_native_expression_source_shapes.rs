//! One shape per expression, cached on the profile's binding catalog; a failure raises.

use crate::analysis_session::_helpers::stage_functions;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;
use pyo3::types::PyTuple;

/// One shape per expression, cached on the profile's binding catalog; a failure raises.
#[inline]
pub fn infer_native_expression_source_shapes<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    stage_functions::infer_native_expression_source_shapes(request)
}
