//! The error of a selector that cannot resolve a named job's nodes.

use pyo3::PyErr;
use pyo3::prelude::Python;
use sqlbuild_analysis::job_settings::errors::JobSelectorError;
use sqlbuild_python_base::records::main::construct::new_error::new_error;
use sqlbuild_python_base::records::types::ErrorId;

/// A `CliUserError` naming the jobs whose selection a selector failed to resolve, with its code.
pub fn job_selector_error(py: Python<'_>, failure: &JobSelectorError) -> PyErr {
    let jobs: String = if failure.jobs.is_empty() {
        "the named jobs".to_owned()
    } else {
        format!("job {}", failure.jobs.join(", "))
    };
    new_error(
        py,
        ErrorId::CliUser,
        &format!(
            "cannot apply the settings of {jobs}: selector '{}' failed: {}",
            failure.selector, failure.error.message
        ),
        (Some(failure.error.code), failure.error.help.as_deref()),
    )
}
