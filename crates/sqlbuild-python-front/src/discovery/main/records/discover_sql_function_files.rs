//! Discover SQL function files under functions/sql/.

use crate::discovery::_helpers::records::declaration_records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// Discover SQL function files under functions/sql/.
#[inline]
pub fn discover_sql_function_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    on_fault: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    declaration_records::discover_sql_function_files(project_dir, on_fault)
}
