//! Resolve the target and its effective database/schema without compiling resources.

use crate::compile_attachment::_helpers::target;
use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::types::{PyDict, PyTuple};

/// Resolve the target and its effective database/schema without compiling resources.
#[inline]
pub fn build_effective_target_namespace<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    target::build_effective_target_namespace(py, options)
}
