//! The project's rules configuration, with the Rules cache off when `cache` is `false`.

use crate::rules::_helpers::configuration::rules_config::config_object;
use crate::rules::main::configuration::rules_config_of::rules_config_of;
use pyo3::Py;
use pyo3::prelude::{PyAny, PyResult, Python};
use sqlbuild_rules::models::RulesConfig;
use std::path::PathBuf;

/// The project's rules configuration, with the Rules cache off when `cache` is `false`.
pub fn load_rules_config_with_cache(
    py: Python<'_>,
    project_dir: PathBuf,
    cache: bool,
) -> PyResult<Py<PyAny>> {
    let mut config: RulesConfig = match rules_config_of(&project_dir)? {
        Ok(config) => config,
        Err(invalid) => return Err(invalid.into_pyerr(py)),
    };
    config.cache.enabled &= cache;
    Ok(config_object(py, &config)?.unbind())
}
