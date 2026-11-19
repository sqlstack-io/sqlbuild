//! The build-wide settings the named jobs give a build.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use serde_json::Value;
use sqlbuild_analysis::job_settings::constants::WAREHOUSE_SETTING;
use sqlbuild_analysis::job_settings::main::build_job_settings::build_job_settings;
use sqlbuild_analysis::job_settings::main::named_jobs::named_jobs;
use sqlbuild_analysis::job_settings::main::without_given_settings::without_given_settings;
use sqlbuild_analysis::job_settings::models::DeclaredRun;
use sqlbuild_config::command_connection::main::check_job_warehouse::check_job_warehouse;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::compile_driver::main::session_warehouse_support::session_warehouse_support;
use sqlbuild_python_front::contracts::main::resolve_effective_adapter_name::resolve_effective_adapter_name;
use sqlbuild_python_front::discovery::main::project::discover_project_configuration::discover_project_configuration;

use crate::job_settings::_helpers::job_run_settings::job_run_settings;
use crate::job_settings::main::job_declarations::job_declarations;
use crate::job_settings::main::job_setting_conflict::job_setting_conflict;
use crate::job_settings::main::with_job_scope::with_job_scope;

/// The warehouse every named job sets, when they all set the same usable one.
fn shared_warehouse(
    py: Python<'_>,
    named: &[String],
    (declared, directory): (&BTreeMap<String, DeclaredRun>, &Bound<'_, PyAny>),
) -> PyResult<Option<Value>> {
    let mut warehouses = named.iter().map(|job| match declared.get(job) {
        Some(DeclaredRun::Job { settings, .. }) => settings.get(WAREHOUSE_SETTING),
        _ => None,
    });
    let Some(Some(warehouse)) = warehouses.next() else {
        return Ok(None);
    };
    if warehouses.any(|other| other != Some(warehouse)) {
        return Ok(None);
    }
    if check_job_warehouse("", warehouse.as_str()).is_err() {
        return Ok(None);
    }
    let discovered: Bound<'_, PyAny> = discover_project_configuration(py, directory)?;
    let adapter_name: Bound<'_, PyAny> = resolve_effective_adapter_name(
        &native_field(&discovered, "project_config")?,
        &native_field(&discovered, "local_config")?,
    )?;
    Ok(session_warehouse_support(py, &adapter_name, directory)?.then(|| (*warehouse).clone()))
}

/// The named jobs' build-wide settings, a shared warehouse included, less `given`; or `S027`.
pub(crate) fn named_job_build_settings(
    py: Python<'_>,
    (selectors, given): (&[String], &BTreeSet<String>),
    (directory, selected_target): (&Path, Option<String>),
) -> PyResult<BTreeMap<String, Value>> {
    let scope = (directory.to_path_buf(), selected_target);
    let Some(declared) = with_job_scope(scope, || job_declarations(py))? else {
        return Ok(BTreeMap::new());
    };
    let declared: BTreeMap<String, DeclaredRun> = without_given_settings(declared, given);
    let named: Vec<String> = named_jobs(selectors, &declared);
    if named.is_empty() {
        return Ok(BTreeMap::new());
    }
    let mut agreed: BTreeMap<String, Value> = build_job_settings(&named, &declared)
        .map_err(|conflict| job_setting_conflict(py, &conflict))?;
    let directory: Bound<'_, PyAny> = directory.into_pyobject(py)?.into_any();
    if let Some(warehouse) = shared_warehouse(py, &named, (&declared, &directory))? {
        agreed.insert(WAREHOUSE_SETTING.to_owned(), warehouse);
    }
    job_run_settings(&directory, agreed)
}
