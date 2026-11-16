//! Write a warning line to stderr for each deprecated setting alias `project_dir` still sets.

use crate::discovery::_helpers::project::project_config::files;
use crate::discovery::_helpers::project::project_config::load_error;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_config::project::main::deprecated_setting_warnings::deprecated_setting_warnings;
use sqlbuild_python_base::native_host::main::write_console::write_console;

/// Write a warning line to stderr for each deprecated setting alias `project_dir` still sets.
pub fn warn_deprecated_settings(py: Python<'_>, project_dir: &Bound<'_, PyAny>) -> PyResult<()> {
    let files = files(project_dir)?;
    match deprecated_setting_warnings(&files.native) {
        Ok(warnings) => warnings
            .iter()
            .try_for_each(|warning| write_console(py, "stderr", &format!("{warning}\n"))),
        Err(failure) => Err(load_error(py, &failure, &files)?),
    }
}
