//! The build-wide settings the named jobs give a build.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use serde_json::Value;
use sqlbuild_analysis::job_settings::constants::WAREHOUSE_SETTING;
use sqlbuild_analysis::job_settings::errors::NodeJobSettingsError;
use sqlbuild_analysis::job_settings::main::resolve_job_settings::resolve_job_settings;
use sqlbuild_analysis::job_settings::models::ResolvedJobSettings;
use sqlbuild_config::command_connection::main::check_job_warehouse::check_job_warehouse;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::compile_driver::main::session_warehouse_support::session_warehouse_support;
use sqlbuild_python_front::contracts::main::resolve_effective_adapter_name::resolve_effective_adapter_name;
use sqlbuild_python_front::discovery::main::project::discover_project_configuration::discover_project_configuration;

use crate::job_settings::_helpers::job_run_settings::job_run_settings;
use crate::job_settings::main::job_declarations::job_declarations;
use crate::job_settings::main::job_selector_error::job_selector_error;
use crate::job_settings::main::job_setting_conflict::job_setting_conflict;
use crate::job_settings::main::with_job_scope::with_job_scope;

/// Whether whole-run `name` = `value` suits a whole build: a valid warehouse the adapter can use.
fn applies_to_whole_run(
    py: Python<'_>,
    (name, value): (&str, &Value),
    directory: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    if name != WAREHOUSE_SETTING || check_job_warehouse("", value.as_str()).is_err() {
        return Ok(false);
    }
    let discovered: Bound<'_, PyAny> = discover_project_configuration(py, directory)?;
    let adapter_name: Bound<'_, PyAny> = resolve_effective_adapter_name(
        &native_field(&discovered, "project_config")?,
        &native_field(&discovered, "local_config")?,
    )?;
    session_warehouse_support(py, &adapter_name, directory)
}

/// The named jobs' build-wide and shared whole-run settings, less `given`; or `S027`.
pub(crate) fn named_job_build_settings(
    py: Python<'_>,
    (selectors, given): (&[String], &BTreeSet<String>),
    (directory, selected_target): (&Path, Option<String>),
) -> PyResult<BTreeMap<String, Value>> {
    let scope = (directory.to_path_buf(), selected_target);
    let Some(declared) = with_job_scope(scope, || job_declarations(py))? else {
        return Ok(BTreeMap::new());
    };
    let resolved: ResolvedJobSettings =
        match resolve_job_settings(declared, (selectors, given), None) {
            Ok(resolved) => resolved,
            Err(NodeJobSettingsError::Conflict(conflict)) => {
                return Err(job_setting_conflict(py, &conflict));
            }
            Err(NodeJobSettingsError::Selector(failure)) => {
                return Err(job_selector_error(py, &failure));
            }
        };
    let directory: Bound<'_, PyAny> = directory.into_pyobject(py)?.into_any();
    let mut agreed: BTreeMap<String, Value> = resolved.build;
    for (name, value) in resolved.whole_run {
        if applies_to_whole_run(py, (&name, &value), &directory)? {
            agreed.insert(name, value);
        }
    }
    job_run_settings(&directory, agreed)
}
