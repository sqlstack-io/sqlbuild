//! `[git] base_ref` of the project at `project_dir`, which `--since @base` names.

use crate::discovery::_helpers::project::project_config;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `[git] base_ref` of the project at `project_dir`, which `--since @base` names.
#[inline]
pub fn configured_git_base_ref(project_dir: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    project_config::configured_git_base_ref(project_dir)
}
