//! Resolve function sql.

use crate::planner_resolution::_helpers::references::orchestration;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;

/// Resolve function sql.
#[inline]
pub fn resolve_function_sql<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    orchestration::resolve_function_sql(py, kwargs)
}
