//! Local-only semantic hashes of functions by name.

use crate::planner_identity::_helpers::metadata;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// Local-only semantic hashes of functions by name.
#[inline]
pub fn build_function_local_hashes<'py>(
    functions: &Bound<'py, PyAny>,
    dialect: Option<&str>,
) -> PyResult<Bound<'py, PyDict>> {
    metadata::build_function_local_hashes(functions, dialect)
}
