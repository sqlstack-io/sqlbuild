//! Compile inputs' canonical index with all usage and placement facts.

use crate::scopes::_helpers::checks::scope_validation;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Compile inputs' canonical index with all usage and placement facts.
#[inline]
pub fn scope_index_with_compile_usages<'py>(
    inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    scope_validation::scope_index_with_compile_usages(inputs)
}
