//! Each query's columns as `NativeQueryColumns`; a native internal failure raises.

use crate::analysis_session::_helpers::stage_functions;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;
use pyo3::types::PyTuple;

/// Each query's columns as `NativeQueryColumns`; a native internal failure raises.
#[inline]
pub fn infer_native_query_columns<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    stage_functions::infer_native_query_columns(request)
}
