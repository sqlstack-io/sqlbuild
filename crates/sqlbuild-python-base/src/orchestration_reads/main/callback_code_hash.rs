//! Include the callable's code even for public in-memory declaration compilation.

use crate::orchestration_reads::_helpers::orchestration_environment;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Include the callable's code even for public in-memory declaration compilation.
#[inline]
pub fn callback_code_hash(function: &Bound<'_, PyAny>, sources: &str) -> PyResult<String> {
    orchestration_environment::callback_code_hash(function, sources)
}
