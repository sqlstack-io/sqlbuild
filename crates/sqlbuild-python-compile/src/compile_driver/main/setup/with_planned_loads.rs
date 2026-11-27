//! A plan pipeline whose loads carry SQLLoad's plan of them, planned through the target's sessions.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced::replaced;
use sqlbuild_python_front::compile_driver::main::planned_load_projects::planned_load_projects;

use crate::compile_driver::main::setup::project_target_settings::project_target_settings;
use crate::compile_driver::models::TargetSettingsValues;

/// `pipeline` with SQLLoad's plan lines on each load its plan runs; a load leaving out a source's table fails.
pub fn with_planned_loads<'py>(
    discovered: &Bound<'py, PyAny>,
    pipeline: &Bound<'py, PyAny>,
    (adapter, connection_config): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = pipeline.py();
    let plan: Bound<'py, PyAny> = native_field(pipeline, "plan_output")?;
    if !native_field(&plan, "load_entries")?.is_truthy()? {
        return Ok(pipeline.clone());
    }
    let project: Bound<'py, PyAny> = native_field(pipeline, "project")?;
    let target: TargetSettingsValues =
        project_target_settings(discovered, &project, (connection_config, adapter))?;
    let target_name: String = match &target.name {
        Some(name) => name.clone(),
        None => native_field(&project, "effective_target_name")?
            .extract::<Option<String>>()?
            .unwrap_or_default(),
    };
    let projects = planned_load_projects(
        discovered,
        (&project, &plan),
        (&target_name, target.loader_schema()),
    )?;
    let planned: Bound<'py, PyAny> = call_with(
        &host_entry(py, ExecutorHost::PlanLoads)?,
        &[
            ("plan", plan),
            ("projects", projects.into_any()),
            ("adapter", adapter.clone()),
            ("connection_config", connection_config.clone()),
            ("base_connection", py.None().into_bound(py)),
            (
                "database",
                pyo3::IntoPyObjectExt::into_bound_py_any(target.database.clone(), py)?,
            ),
        ],
    )?;
    replaced(pipeline, &[("plan_output", planned)])
}
