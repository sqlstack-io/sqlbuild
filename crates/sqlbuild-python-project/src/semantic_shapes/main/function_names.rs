//! Each declared function's name and its UDF and table-function analysis names, sorted.

use crate::semantic_shapes::_helpers::declarations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Each declared function's name and its UDF and table-function analysis names, sorted.
#[inline]
pub fn function_names(functions: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    declarations::function_names(functions)
}
