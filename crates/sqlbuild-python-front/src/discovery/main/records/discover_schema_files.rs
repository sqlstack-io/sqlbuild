//! Discover model schema.yml and seed declaration files natively.

use crate::discovery::_helpers::records::yaml_records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// Discover model schema.yml and seed declaration files natively.
#[inline]
pub fn discover_schema_files<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    yaml_records::discover_schema_files(project_dir)
}
