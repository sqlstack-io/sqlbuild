//! `validate_local_sql_analysis_policy` from the project requirement and the local setting.

use crate::discovery::_helpers::project::project_config::config_error;
use crate::discovery::_helpers::project::project_config::files;
use pyo3::prelude::{Bound, PyAny, PyErr, Python};
use pyo3::pyfunction;
use sqlbuild_config::project::main::local_sql_analysis_failure::local_sql_analysis_failure;

/// `validate_local_sql_analysis_policy` from the project requirement and the local setting.
#[pyfunction]
pub fn validate_local_sql_analysis(
    py: Python<'_>,
    project_dir: &Bound<'_, PyAny>,
    flags: (bool, bool, bool),
) -> Result<(), PyErr> {
    let files = files(project_dir)?;
    match local_sql_analysis_failure(&files.native, flags) {
        None => Ok(()),
        Some(failure) => Err(config_error(py, &failure, &files)?),
    }
}
