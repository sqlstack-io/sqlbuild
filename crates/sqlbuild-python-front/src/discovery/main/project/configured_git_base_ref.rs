//! `[git] base_ref` of the project at `project_dir`, which `--since @base` names.

use crate::discovery::_helpers::project::project_config::configured;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_config::project::main::load_git_base_ref::load_git_base_ref;

/// `[git] base_ref` of the project at `project_dir`, which `--since @base` names.
pub fn configured_git_base_ref(project_dir: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    configured(project_dir, load_git_base_ref)
}
