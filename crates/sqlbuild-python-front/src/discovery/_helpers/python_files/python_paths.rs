//! Reject project Python outside documented executable roots.

use std::path::PathBuf;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_discovery::python_paths::main::unsupported_python_paths::unsupported_python_paths;

use crate::discovery::_helpers::failures::raised::discovery_error;

const PYTHON_PATH_HELP: &str = "Move pipeline Python nodes and the helper modules they import under \
python/, or move SQLBuild extensions under their documented extension path.";

/// Raise `ProjectPythonPathError` naming every unsupported Python file below `project_dir`.
pub(crate) fn reject_unsupported_python_paths(project_dir: &Bound<'_, PyAny>) -> PyResult<()> {
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
