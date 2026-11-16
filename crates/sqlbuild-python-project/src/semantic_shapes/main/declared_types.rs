//! Types named by relation shapes and function signatures, sorted, `UNKNOWN` left out.

use crate::semantic_shapes::_helpers::declarations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Types named by relation shapes and function signatures, sorted, `UNKNOWN` left out.
#[inline]
pub fn declared_types(
    functions: &Bound<'_, PyAny>,
    column_types: &Bound<'_, PyAny>,
) -> PyResult<Vec<String>> {
    declarations::declared_types(functions, column_types)
}
