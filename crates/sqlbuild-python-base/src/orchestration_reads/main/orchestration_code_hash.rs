//! Hash declaration/helper sources once per compilation, never once per sensor.

use crate::orchestration_reads::_helpers::orchestration_environment;
use pyo3::prelude::PyResult;
use std::path::Path;

/// Hash declaration/helper sources once per compilation, never once per sensor.
#[inline]
pub fn orchestration_code_hash(project_dir: &Path) -> PyResult<String> {
    orchestration_environment::orchestration_code_hash(project_dir)
}
