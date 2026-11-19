//! `[maintained] heartbeat_grace` of the project at `project_dir`, in seconds.

use crate::discovery::_helpers::project::project_config::configured;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_config::project::main::load_maintained_config::load_maintained_config;

/// Seconds a pausing build may be silent before its maintained models are resumed.
pub fn configured_heartbeat_grace(project_dir: &Bound<'_, PyAny>) -> PyResult<u64> {
    Ok(configured(project_dir, load_maintained_config)?.heartbeat_grace_seconds)
}
