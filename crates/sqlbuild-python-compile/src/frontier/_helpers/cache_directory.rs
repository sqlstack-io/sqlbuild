//! The project-local compiler store root.

use crate::frontier::main::cache_directory::cache_directory;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;

/// Return the project's compiler store root.
#[pyfunction]
pub(crate) fn compiler_cache_directory<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    cache_directory(project_dir)
}
