//! The selectors that select a frozen run's nodes back.

use std::collections::BTreeMap;

use pyo3::prelude::PyResult;
use sqlbuild_orchestration::runs::models::RunRequest;

use crate::job_settings::_helpers::frozen_selector::frozen_key_selectors;

/// One selector per frozen node the run has not skipped.
pub fn frozen_selectors(
    frozen: &RunRequest,
    skipped_nodes: &BTreeMap<String, String>,
) -> PyResult<Vec<String>> {
    frozen_key_selectors(&frozen.selection, skipped_nodes)
}
