//! Detect a project-level python/adapter.py without importing it.

use crate::discovery::_helpers::records::declaration_records::path_class;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_core::constants::ADAPTER_FILE_PATH;
use sqlbuild_python_base::records::types::RecordId;

/// Detect a project-level python/adapter.py without importing it.
#[pyfunction]
#[pyo3(signature = (*, project_dir))]
pub fn discover_adapter_file<'py>(project_dir: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    let file_path: Bound<'py, PyAny> = project_dir.div(ADAPTER_FILE_PATH)?;
    if !file_path.call_method0("is_file")?.is_truthy()? {
        return Ok(py.None().into_bound(py));
    }
    new_record_with(
        py,
        RecordId::DiscoveredAdapterFile,
        &[
            ("file_path", file_path),
            ("relative_path", path_class(py)?.call1((ADAPTER_FILE_PATH,))?),
        ],
    )
}
