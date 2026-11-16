//! Detect changes and build a plan entry and warnings for a single model.

use crate::planner_entries::_helpers::entries::model_entries;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;

/// Detect changes and build a plan entry and warnings for a single model.
#[inline]
pub fn plan_model<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    model_entries::plan_model(py, kwargs)
}
