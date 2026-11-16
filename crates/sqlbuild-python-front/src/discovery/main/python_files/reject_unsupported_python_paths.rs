//! Raise `ProjectPythonPathError` naming every unsupported Python file below `project_dir`.

use crate::discovery::_helpers::failures::raised::discovery_error;
use crate::discovery::_helpers::python_files::python_paths::PYTHON_PATH_HELP;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_discovery::python_paths::main::unsupported_python_paths::unsupported_python_paths;
use std::path::PathBuf;

/// Raise `ProjectPythonPathError` naming every unsupported Python file below `project_dir`.
pub fn reject_unsupported_python_paths(project_dir: &Bound<'_, PyAny>) -> PyResult<()> {
    let root: PathBuf = project_dir.extract()?;
    let unsupported: Vec<String> = unsupported_python_paths(&root)
        .into_iter()
        .map(|parts| parts.join(std::path::MAIN_SEPARATOR_STR))
        .collect();
    if unsupported.is_empty() {
        return Ok(());
    }
    Err(discovery_error(
        project_dir.py(),
        "python_path",
        format!(
            "Unsupported project Python path(s): {}",
            unsupported.join(", ")
        ),
        Some(PYTHON_PATH_HELP.to_owned()),
    ))
}
