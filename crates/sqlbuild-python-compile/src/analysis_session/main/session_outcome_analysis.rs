//! The outcome at `position` of a finished session as `ModelSqlAnalysis`, for hand-built inputs.

use crate::analysis_session::_helpers::outcome_objects;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// The outcome at `position` of a finished session as `ModelSqlAnalysis`, for hand-built inputs.
#[inline]
pub fn session_outcome_analysis<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    outcome_objects::session_outcome_analysis(request)
}
