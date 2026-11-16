//! Planned write hashes replaced by hashes based on upstreams available this run.

use crate::planner_identity::_helpers::identities;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;

/// Planned write hashes replaced by hashes based on upstreams available this run.
#[inline]
pub fn with_honest_model_write_hashes<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    identities::with_honest_model_write_hashes(py, options)
}
