//! The project or local configuration file path, rejecting an unconverted legacy file.

use crate::discovery::_helpers::project::project_config::config_error;
use crate::discovery::_helpers::project::project_config::file_index;
use crate::discovery::_helpers::project::project_config::files;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_config::project::main::resolve_config_file::resolve_config_file;

/// The project or local configuration file path, rejecting an unconverted legacy file.
#[pyfunction]
pub fn resolve_config_file_path<'py>(
    py: Python<'py>,
    project_dir: &Bound<'py, PyAny>,
    local: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let files = files(project_dir)?;
    match resolve_config_file(&files.native, local) {
        Ok(file) => Ok(files.python[file_index(file)].clone()),
        Err(failure) => Err(config_error(py, &failure, &files)?),
    }
}
