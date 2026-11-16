//! `[review]` of the project at `project_dir`: the defaults `sqb review` falls back to.

use crate::discovery::_helpers::project::project_config;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_config::project::models::ReviewConfig;

/// `[review]` of the project at `project_dir`: the defaults `sqb review` falls back to.
#[inline]
pub fn configured_review(project_dir: &Bound<'_, PyAny>) -> PyResult<ReviewConfig> {
    project_config::configured_review(project_dir)
}
