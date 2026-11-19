//! The build-wide settings the named jobs give a build.

use std::collections::BTreeMap;
use std::path::Path;

use pyo3::IntoPyObject;
use pyo3::prelude::{PyResult, Python};
use serde_json::Value;
use sqlbuild_analysis::job_settings::main::build_job_settings::build_job_settings;
use sqlbuild_analysis::job_settings::main::named_jobs::named_jobs;

use crate::job_settings::_helpers::job_run_settings::job_run_settings;
use crate::job_settings::main::job_declarations::job_declarations;
use crate::job_settings::main::job_setting_conflict::job_setting_conflict;
use crate::job_settings::main::with_job_scope::with_job_scope;

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
