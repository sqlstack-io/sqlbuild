//! `[changes]` of the project at `project_dir`: what `--changed` counts and reaches by default.

use crate::discovery::_helpers::project::project_config::configured;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_config::project::main::load_changes_config::load_changes_config;
use sqlbuild_config::project::models::ChangesConfig;

/// `[changes]` of the project at `project_dir`: what `--changed` counts and reaches by default.
pub fn configured_changes(project_dir: &Bound<'_, PyAny>) -> PyResult<ChangesConfig> {
    configured(project_dir, load_changes_config)
}
