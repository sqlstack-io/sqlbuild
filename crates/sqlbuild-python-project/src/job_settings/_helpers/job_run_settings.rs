//! A job run's `--changed` settings.

use std::collections::BTreeMap;

use pyo3::prelude::{Bound, PyAny, PyResult};
use serde_json::Value;
use sqlbuild_python_front::discovery::main::project::configured_changes::configured_changes;

use crate::compiled_project::constants::{
    CHANGED_SETTING, JOB_RUN_COUNTS, JOB_RUN_REACH, REACH_SETTING,
};

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
