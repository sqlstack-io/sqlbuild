//! The project's validated rules configuration as `RulesConfig`.

use crate::rules::main::configuration::load_rules_config_with_cache::load_rules_config_with_cache;
use pyo3::prelude::{PyAny, PyResult, Python};
use pyo3::{Py, pyfunction};
use std::path::PathBuf;

/// The project's validated rules configuration as `RulesConfig`.
#[pyfunction]
#[pyo3(signature = (*, project_dir))]
pub fn load_rules_config(py: Python<'_>, project_dir: PathBuf) -> PyResult<Py<PyAny>> {
    load_rules_config_with_cache(py, project_dir, true)
}
