//! The selector that selects one frozen node back, as a job run selects it.

use std::collections::{BTreeMap, BTreeSet};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::definition_changes::constants::PYTHON_NODE_KINDS;

use crate::job_settings::constants::SQL_TEST_NODE_KIND;

/// A Python node by its key, a SQL unit test by `test:`, any other node by name, so a source also selects its loader.
pub(crate) fn frozen_selector(key: &str) -> PyResult<String> {
    let (kind, name) = key
        .split_once(':')
        .ok_or_else(|| PyValueError::new_err("frozen run contains an invalid node key"))?;
    Ok(if PYTHON_NODE_KINDS.contains(&kind) {
        key.to_owned()
    } else if kind == SQL_TEST_NODE_KIND {
        format!("test:{name}")
    } else {
        name.to_owned()
    })
}

/// One selector per frozen key that `skipped` does not name, in key order.
pub(crate) fn frozen_key_selectors(
    keys: &BTreeSet<String>,
    skipped: &BTreeMap<String, String>,
) -> PyResult<Vec<String>> {
    keys.iter()
        .filter(|key| !skipped.contains_key(*key))
        .map(|key| frozen_selector(key))
        .collect()
}
