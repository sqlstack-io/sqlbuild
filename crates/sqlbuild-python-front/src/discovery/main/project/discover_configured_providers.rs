//! The configuration with discovery's checks and warnings, and the provider classes, without importing any other project Python.

use crate::compile_driver::main::reject_invalid_project_configuration::reject_invalid_project_configuration;
use crate::discovery::_helpers::project::configuration::loaded_configuration;
use crate::discovery::_helpers::python_files::imported_providers::discover_provider_classes;
use crate::discovery::main::project::warn_deprecated_settings::warn_deprecated_settings;
use crate::discovery::models::SecretPolicy;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// The configuration with discovery's checks and warnings, and the provider classes, without importing any other project Python.
pub fn discover_configured_providers<'py>(
    py: Python<'py>,
    project_dir: &Bound<'py, PyAny>,
    secret_policy: SecretPolicy,
) -> PyResult<Bound<'py, PyAny>> {
    let configuration: Bound<'py, PyAny> = loaded_configuration(py, project_dir, |config| {
        reject_invalid_project_configuration(config)?;
        warn_deprecated_settings(py, project_dir)
    })?;
    let providers: Bound<'py, PyAny> =
        discover_provider_classes(project_dir.clone(), secret_policy.name())?;
    new_record_with(
        py,
        RecordId::DiscoveredProjectInputs,
        &[
            ("project_dir", project_dir.clone()),
            (
                "project_config",
                native_field(&configuration, "project_config")?,
            ),
            (
                "local_config",
                native_field(&configuration, "local_config")?,
            ),
            ("providers", providers),
        ],
    )
}
