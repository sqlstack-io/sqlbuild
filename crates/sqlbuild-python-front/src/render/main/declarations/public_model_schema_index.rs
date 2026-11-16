//! Every public model schema by name, with its inherited columns first, collision-checked.

use crate::render::_helpers::declarations::declaration_indexes;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// Every public model schema by name, with its inherited columns first, collision-checked.
#[inline]
pub fn public_model_schema_index<'py>(inputs: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    declaration_indexes::public_model_schema_index(inputs)
}
