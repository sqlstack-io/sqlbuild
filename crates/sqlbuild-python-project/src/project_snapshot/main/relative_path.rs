//! Relative path.

use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_base::records::main::fields::path_posix::path_posix;

/// Relative path.
pub fn relative_path(value: &Bound<'_, PyAny>) -> PyResult<String> {
    path_posix(&field(value, "relative_path")?)
}
