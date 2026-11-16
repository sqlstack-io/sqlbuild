//! The configuration with discovery's checks and warnings, and the provider classes, without importing any other project Python.

use crate::discovery::_helpers::project::configuration;
use crate::discovery::models::SecretPolicy;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The configuration with discovery's checks and warnings, and the provider classes, without importing any other project Python.
#[inline]
pub fn discover_configured_providers<'py>(
    py: Python<'py>,
    project_dir: &Bound<'py, PyAny>,
    secret_policy: SecretPolicy,
) -> PyResult<Bound<'py, PyAny>> {
    configuration::discover_configured_providers(py, project_dir, secret_policy)
}
