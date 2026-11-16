//! Discover source YAML files natively, reporting failing files to `on_fault`.

use crate::discovery::_helpers::records::yaml_records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// Discover source YAML files natively, reporting failing files to `on_fault`.
#[inline]
pub fn discover_source_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    on_fault: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    yaml_records::discover_source_files(project_dir, on_fault)
}
