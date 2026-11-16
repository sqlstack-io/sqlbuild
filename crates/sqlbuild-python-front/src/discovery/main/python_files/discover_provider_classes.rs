//! Provider classes under providers/.

use crate::discovery::_helpers::python_files::imported_providers;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Provider classes under providers/.
#[inline]
pub fn discover_provider_classes<'py>(
    project_dir: Bound<'py, PyAny>,
    secret_policy: &str,
) -> PyResult<Bound<'py, PyAny>> {
    imported_providers::discover_provider_classes(project_dir, secret_policy)
}
