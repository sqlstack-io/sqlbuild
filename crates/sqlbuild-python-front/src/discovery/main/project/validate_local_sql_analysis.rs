//! `validate_local_sql_analysis_policy` from the project requirement and the local setting.

use crate::discovery::_helpers::project::project_config;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyErr;
use pyo3::prelude::Python;

/// `validate_local_sql_analysis_policy` from the project requirement and the local setting.
#[inline]
pub fn validate_local_sql_analysis(
    py: Python<'_>,
    project_dir: &Bound<'_, PyAny>,
    flags: (bool, bool, bool),
) -> Result<(), PyErr> {
    project_config::validate_local_sql_analysis(py, project_dir, flags)
}
