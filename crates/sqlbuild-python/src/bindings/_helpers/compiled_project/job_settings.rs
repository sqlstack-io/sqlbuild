//! The settings named jobs give a command: the build-wide ones as the flags the command line left out.

use std::collections::BTreeMap;
use std::path::Path;

use pyo3::IntoPyObject;
use pyo3::PyErr;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use serde_json::Value;
use sqlbuild_analysis::graph::main::parse_selector::parse_selector;
use sqlbuild_analysis::graph::models::ParsedSelector;
use sqlbuild_analysis::job_settings::constants::JOB_SETTING_CONFLICT_CODE;
use sqlbuild_analysis::job_settings::errors::{JobSelectorError, JobSettingConflict};
use sqlbuild_analysis::job_settings::main::build_job_settings::build_job_settings;
use sqlbuild_analysis::job_settings::main::named_jobs::named_jobs;

use sqlbuild_python_base::records::main::construct::new_error::new_error;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_front::discovery::main::project::configured_changes::configured_changes;
use sqlbuild_python_project::compiled_project::constants::{
    CHANGED_SETTING, JOB_RUN_COUNTS, JOB_RUN_REACH, JOB_SELECTOR_KINDS, REACH_SETTING,
    SELECTOR_INTERSECTION,
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

/// The build-wide settings the named jobs give a build, as their runs take them, or `S027`.
pub(crate) fn named_job_build_settings(
    py: Python<'_>,
    selectors: &[String],
    (directory, selected_target): (&Path, Option<String>),
) -> PyResult<BTreeMap<String, Value>> {
    let scope = (directory.to_path_buf(), selected_target);
    let Some(declared) = with_job_scope(scope, || job_declarations(py))? else {
        return Ok(BTreeMap::new());
    };
    let named: Vec<String> = named_jobs(selectors, &declared);
    if named.is_empty() {
        return Ok(BTreeMap::new());
    }
    let agreed: BTreeMap<String, Value> = build_job_settings(&named, &declared)
        .map_err(|conflict| job_setting_conflict(py, &conflict))?;
    job_run_settings(&directory.into_pyobject(py)?.into_any(), agreed)
}

/// Whether a selector has an atom that can name a job: `job:<name>`, or a bare name.
pub(crate) fn could_name_job(selector: &str) -> bool {
    selector
        .split_whitespace()
        .flat_map(|token| token.split(SELECTOR_INTERSECTION))
        .any(|atom| match parse_selector(atom) {
            Ok(ParsedSelector::Kind { kind, .. }) => JOB_SELECTOR_KINDS.contains(&kind.as_str()),
            _ => false,
        })
}

/// `CliUserError` for job settings that cannot apply together.
pub(crate) fn job_setting_conflict(py: Python<'_>, conflict: &JobSettingConflict) -> PyErr {
    new_error(
        py,
        ErrorId::CliUser,
        &conflict.message,
        (Some(JOB_SETTING_CONFLICT_CODE), Some(conflict.help)),
    )
}

/// A `CliUserError` naming the jobs whose selection a selector failed to resolve, with its code.
pub(crate) fn job_selector_error(py: Python<'_>, failure: &JobSelectorError) -> PyErr {
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
