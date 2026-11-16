//! Every public enum and constant by name, collision-checked.

use crate::render::_helpers::declarations::declaration_indexes;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// Every public enum and constant by name, collision-checked.
#[inline]
pub fn public_declaration_indexes<'py>(
    inputs: Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyDict>, Bound<'py, PyDict>)> {
    declaration_indexes::public_declaration_indexes(inputs)
}
