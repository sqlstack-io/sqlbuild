//! Create the blank project tree, raising the `OSError` Python's `pathlib` would.

use std::io;
use std::path::Path;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use sqlbuild_core::init_command::errors::ScaffoldError;
use sqlbuild_core::init_command::main::scaffold_blank_project::scaffold_blank_project as scaffold;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `OSError(errno, strerror, filename)`, which Python narrows to its errno subclass.
fn os_error(py: Python<'_>, error: &io::Error, path: &Path) -> PyErr {
    let raised: PyResult<Bound<'_, PyAny>> = match error.raw_os_error() {
        Some(errno) => {
            let described: String = io::Error::from_raw_os_error(errno).to_string();
            let strerror: &str = described
                .strip_suffix(&format!(" (os error {errno})"))
                .unwrap_or(&described);
            py.import("builtins").and_then(|builtins| {
                native_field(&builtins, "OSError")?.call1((
                    errno,
                    strerror,
                    path.to_string_lossy().into_owned(),
                ))
            })
        }
        None => py
            .import("builtins")
            .and_then(|builtins| native_field(&builtins, "OSError")?.call1((error.to_string(),))),
    };
    match raised {
        Ok(raised) => PyErr::from_value(raised),
        Err(failure) => failure,
    }
}

/// Create the resource directories, the project file, and `.gitkeep` in empty directories.
pub(crate) fn scaffold_blank_project(
    py: Python<'_>,
    base_dir: &Path,
    project_toml: &str,
) -> PyResult<()> {
    scaffold(base_dir, project_toml)
        .map_err(|ScaffoldError { error, path }| os_error(py, &error, &path))
}
