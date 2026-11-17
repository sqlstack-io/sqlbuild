//! `[changes]` of the project at `project_dir`: what `--changed` counts and reaches by default.

use crate::discovery::_helpers::project::project_config;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_config::project::models::ChangesConfig;

/// `[changes]` of the project at `project_dir`: what `--changed` counts and reaches by default.
#[inline]
pub fn configured_changes(project_dir: &Bound<'_, PyAny>) -> PyResult<ChangesConfig> {
    project_config::configured_changes(project_dir)
}
