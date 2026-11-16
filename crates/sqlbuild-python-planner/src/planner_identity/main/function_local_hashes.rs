//! Local hashes of functions by name; unchanged functions keep the latest local hash `recorded` models saw.

use crate::planner_identity::_helpers::metadata;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyDict;

/// Local hashes of functions by name; unchanged functions keep the latest local hash `recorded` models saw.
#[inline]
pub fn function_local_hashes<'py>(
    functions: &Bound<'py, PyAny>,
    dialect: Option<&str>,
    recorded: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    metadata::function_local_hashes(functions, dialect, recorded)
}
