//! The project's validated rules configuration, or why it is invalid.

use crate::rules::models::InvalidRulesConfig;
use pyo3::prelude::PyResult;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_rules::configuration::errors::RulesConfigLoadError;
use sqlbuild_rules::configuration::main::load_rules_config::load_rules_config as load_config;
use sqlbuild_rules::models::RulesConfig;
use std::path::{Path, PathBuf};

/// The project's validated rules configuration, or why it is invalid.
pub fn rules_config_of(project_dir: &Path) -> PyResult<Result<RulesConfig, InvalidRulesConfig>> {
    let project_dir: PathBuf = project_dir
        .canonicalize()
        .or_else(|_| std::path::absolute(project_dir))?;
    match compiler_guard(|| Ok(load_config(&project_dir)))? {
        Ok(config) => Ok(Ok(config)),
        Err(RulesConfigLoadError::Invalid(message)) => {
            Ok(Err(InvalidRulesConfig::Unreadable(message)))
        }
        Err(RulesConfigLoadError::Policy(error)) => Ok(Err(InvalidRulesConfig::Forbidden {
            message: error.message,
            help: error.help,
        })),
    }
}
