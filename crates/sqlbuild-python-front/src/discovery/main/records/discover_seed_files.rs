//! Discover seed CSV files under seeds/.

use crate::discovery::_helpers::records::declaration_records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// Discover seed CSV files under seeds/.
#[inline]
pub fn discover_seed_files<'py>(project_dir: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    declaration_records::discover_seed_files(project_dir)
}
