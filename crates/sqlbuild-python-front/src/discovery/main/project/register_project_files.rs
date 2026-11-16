//! Whether native discovery reproduces the string semantics of this Python and Unicode version.

use crate::discovery::_helpers::project::project_files;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Whether native discovery reproduces the string semantics of this Python and Unicode version.
#[inline]
pub fn register_project_files(module: &Bound<'_, PyModule>) -> PyResult<()> {
    project_files::register(module)
}
