//! The calendars and sensors one module defines; only module execution crosses into Python.

use crate::orchestration_reads::_helpers::orchestration_declarations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use std::path::Path;

/// The calendars and sensors one module defines; only module execution crosses into Python.
#[inline]
pub fn module_declarations<'py>(
    py: Python<'py>,
    file_path: &Path,
    project_dir: &Path,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    orchestration_declarations::module_declarations(py, file_path, project_dir)
}
