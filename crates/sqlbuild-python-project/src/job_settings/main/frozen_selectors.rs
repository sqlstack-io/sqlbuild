//! The selectors that select a frozen run's nodes back.

use std::collections::BTreeMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::definition_changes::constants::PYTHON_NODE_KINDS;
use sqlbuild_orchestration::runs::models::RunRequest;

use crate::job_settings::constants::SQL_TEST_NODE_KIND;

/// One selector per frozen node the run has not skipped.
pub fn frozen_selectors(
    frozen: &RunRequest,
    skipped_nodes: &BTreeMap<String, String>,
) -> PyResult<Vec<String>> {
    let mut selectors: Vec<String> = Vec::new();
    for key in frozen
        .selection
        .iter()
        .filter(|key| !skipped_nodes.contains_key(*key))
    {
        let (kind, name) = key
            .split_once(':')
            .ok_or_else(|| PyValueError::new_err("frozen run contains an invalid node key"))?;
        selectors.push(if PYTHON_NODE_KINDS.contains(&kind) {
            key.clone()
        } else if kind == SQL_TEST_NODE_KIND {
            format!("test:{name}")
        } else {
            name.to_owned()
        });
    }
    Ok(selectors)
}
