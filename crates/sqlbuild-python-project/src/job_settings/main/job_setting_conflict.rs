//! The error of job settings that cannot apply together.

use pyo3::PyErr;
use pyo3::prelude::Python;
use sqlbuild_analysis::job_settings::constants::JOB_SETTING_CONFLICT_CODE;
use sqlbuild_analysis::job_settings::errors::JobSettingConflict;
use sqlbuild_python_base::records::main::construct::new_error::new_error;
use sqlbuild_python_base::records::types::ErrorId;

/// `CliUserError` for job settings that cannot apply together.
pub fn job_setting_conflict(py: Python<'_>, conflict: &JobSettingConflict) -> PyErr {
    new_error(
        py,
        ErrorId::CliUser,
        &conflict.message,
        (Some(JOB_SETTING_CONFLICT_CODE), Some(conflict.help)),
    )
}
