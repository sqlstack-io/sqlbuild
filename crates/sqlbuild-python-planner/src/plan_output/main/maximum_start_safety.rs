//! Automatic-start policy and physical eligibility evidence, or `None`.

use crate::plan_output::_helpers::safety;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Automatic-start policy and physical eligibility evidence, or `None`.
#[inline]
pub fn maximum_start_safety<'py>(evidence: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    safety::maximum_start_safety(evidence)
}
