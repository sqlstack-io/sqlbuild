//! `[review]` of the project at `project_dir`: the defaults `sqb review` falls back to.

use crate::discovery::_helpers::project::project_config::configured;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_config::project::main::load_review_config::load_review_config;
use sqlbuild_config::project::models::ReviewConfig;

/// `[review]` of the project at `project_dir`: the defaults `sqb review` falls back to.
pub fn configured_review(project_dir: &Bound<'_, PyAny>) -> PyResult<ReviewConfig> {
    configured(project_dir, load_review_config)
}
