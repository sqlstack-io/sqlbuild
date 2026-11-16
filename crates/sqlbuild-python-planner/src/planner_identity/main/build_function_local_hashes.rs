//! Local-only semantic hashes of functions by name.

use crate::planner_identity::_helpers::metadata::function_local_hashes;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyDict;

/// Local-only semantic hashes of functions by name.
#[pyfunction]
#[pyo3(signature = (*, functions, dialect))]
pub fn build_function_local_hashes<'py>(
    functions: &Bound<'py, PyAny>,
    dialect: Option<&str>,
) -> PyResult<Bound<'py, PyDict>> {
    function_local_hashes(
        functions,
        dialect,
        &functions.py().None().into_bound(functions.py()),
    )
}
