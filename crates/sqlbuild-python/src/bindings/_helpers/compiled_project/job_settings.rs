//! The settings named jobs give a command: the build-wide ones as the flags the command line left out.

use std::collections::BTreeMap;
use std::path::PathBuf;

use pyo3::PyErr;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use serde_json::Value;
use sqlbuild_analysis::job_settings::constants::JOB_SETTING_CONFLICT_CODE;
use sqlbuild_analysis::job_settings::errors::JobSettingConflict;
use sqlbuild_analysis::job_settings::main::build_job_settings::build_job_settings;
use sqlbuild_analysis::job_settings::main::named_jobs::named_jobs;

use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_front::discovery::main::project::configured_changes::configured_changes;
use sqlbuild_python_project::compiled_project::constants::{
    CHANGED_SETTING, JOB_RUN_COUNTS, JOB_RUN_REACH, REACH_SETTING,
};
use sqlbuild_python_project::compiled_project::main::job_declarations::job_declarations;
use sqlbuild_python_project::compiled_project::main::with_job_scope::with_job_scope;

/// A job run's `--changed` settings: its whole selection, and source data counted unless configured.
pub(crate) fn job_run_settings(
    project_dir: &Bound<'_, PyAny>,
    mut settings: BTreeMap<String, Value>,
) -> PyResult<BTreeMap<String, Value>> {
    let Some(changed) = settings.get(CHANGED_SETTING).cloned() else {
        return Ok(settings);
    };
    if changed == Value::Bool(false) {
        return Ok(settings);
    }
    settings.insert(REACH_SETTING.to_owned(), Value::from(JOB_RUN_REACH));
    if changed == Value::Bool(true) && configured_changes(project_dir)?.counts.is_none() {
        settings.insert(
            CHANGED_SETTING.to_owned(),
            Value::from(JOB_RUN_COUNTS.to_vec()),
        );
    }
    Ok(settings)
}

/// The build-wide settings the jobs `selectors` name give a build, as job runs take them; a
/// conflict between named jobs is a `CliUserError` naming them.
pub(crate) fn named_job_build_settings(
    py: Python<'_>,
    selectors: &[String],
    (project_dir, selected_target): (&Bound<'_, PyAny>, Option<String>),
) -> PyResult<BTreeMap<String, Value>> {
    let directory: PathBuf = project_dir.extract()?;
    let Some(declared) = with_job_scope((directory, selected_target), || job_declarations(py))?
    else {
        return Ok(BTreeMap::new());
    };
    let named: Vec<String> = named_jobs(selectors, &declared);
    if named.is_empty() {
        return Ok(BTreeMap::new());
    }
    let agreed: BTreeMap<String, Value> = build_job_settings(&named, &declared)
        .map_err(|conflict| job_setting_conflict(py, &conflict))?;
    job_run_settings(project_dir, agreed)
}

/// `CliUserError` for job settings that cannot apply together.
pub(crate) fn job_setting_conflict(py: Python<'_>, conflict: &JobSettingConflict) -> PyErr {
    let raised = (|| {
        let keywords: Bound<'_, PyDict> = PyDict::new(py);
        keywords.set_item("code", JOB_SETTING_CONFLICT_CODE)?;
        keywords.set_item("help", conflict.help)?;
        error_class(py, ErrorId::CliUser)?.call((conflict.message.as_str(),), Some(&keywords))
    })();
    match raised {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}
