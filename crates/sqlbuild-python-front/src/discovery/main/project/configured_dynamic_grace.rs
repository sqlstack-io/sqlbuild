//! How long a dynamic-model hold may go without a heartbeat: the project's `[dynamic]` grace.

use crate::discovery::_helpers::project::project_config::configured;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_config::project::main::load_dynamic_config::load_dynamic_config;

/// `[dynamic] heartbeat_grace` in seconds for the project in `project_dir`.
pub fn configured_dynamic_grace(project_dir: &Bound<'_, PyAny>) -> PyResult<u64> {
    Ok(configured(project_dir, load_dynamic_config)?.heartbeat_grace_seconds)
}
