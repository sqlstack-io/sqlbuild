//! A completed sync's only warehouse write: resume the dynamic models whose holds expired.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDict;
use serde_json::Value;
use sqlbuild_model_config::model_validation::constants::DYNAMIC_ADAPTERS;
use sqlbuild_orchestration::warehouse_metadata::constants::DYNAMIC_RESUME_RECORDER;
use sqlbuild_orchestration::warehouse_metadata::main::dynamic_resume::resume_locations::resume_locations;
use sqlbuild_orchestration::warehouse_metadata::models::{
    DynamicResumeLocation, DynamicResumed, ResumedHold, WarehouseStateLocation,
};
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_compile::compile_driver::main::setup::selected_target_settings::selected_target_settings;
use sqlbuild_python_front::compile_attachment::main::build_effective_runtime_config::build_effective_runtime_config;
use sqlbuild_python_front::native_host::main::resume_expired_dynamic_holds::resume_expired_dynamic_holds;
use sqlbuild_python_front::native_host::types::ResumeLocation;

use crate::bindings::_helpers::command_runtime::connection::project_connection_config;
use crate::bindings::_helpers::location_command::command::location_arguments;
use crate::bindings::_helpers::query_command::project::{
    ProjectAdapter, ProjectInputs, project_adapter,
};

/// Resume expired holds in `locations` and the target's pause table; no session without dynamic models.
pub(super) fn runtime_resume(
    py: Python<'_>,
    options: &Bound<'_, PyDict>,
    locations: &Value,
) -> PyResult<DynamicResumed> {
    let requested: Vec<DynamicResumeLocation> = serde_json::from_value(locations.clone())
        .map_err(|_| PyValueError::new_err("runtime received an invalid dynamic resume"))?;
    let arguments = location_arguments(py, Some(options.clone()))?;
    let ProjectAdapter {
        project_dir,
        discovered,
        adapter,
    } = project_adapter(
        py,
        Some(arguments.value("project_dir")?),
        ProjectInputs::Configuration,
    )?;
    let adapter_name: String = field(&adapter, "adapter_name")?.str()?.extract()?;
    let dynamic: bool = DYNAMIC_ADAPTERS.contains(&adapter_name.as_str());
    let target: Bound<'_, PyAny> = arguments.value("selected_target")?;
    let config: Bound<'_, PyAny> = project_connection_config(
        &discovered,
        (&project_dir, &target),
        (&py.None().into_bound(py), true),
        false,
    )?;
    let pause_location: Option<WarehouseStateLocation> = match dynamic {
        true => pause_location((&discovered, &target), &adapter_name, (&config, &adapter))?,
        false => None,
    };
    let locations: Vec<ResumeLocation> = resume_locations(requested, (dynamic, pause_location))
        .into_iter()
        .map(|resume| {
            (
                (resume.location.database, resume.location.schema),
                resume.quiet_token,
            )
        })
        .collect();
    if locations.is_empty() {
        return Ok(DynamicResumed {
            holds: Vec::new(),
            quiet_tokens: Vec::new(),
            failures: Vec::new(),
        });
    }
    let recorded_by = format!(
        "{DYNAMIC_RESUME_RECORDER}{}",
        arguments.value("location_id")?.str()?
    );
    let (holds, quiet_tokens, failures) =
        resume_expired_dynamic_holds((&adapter, &config), &project_dir, &locations, &recorded_by)?;
    Ok(DynamicResumed {
        holds: holds
            .into_iter()
            .map(|(run_id, relation)| ResumedHold { run_id, relation })
            .collect(),
        quiet_tokens: quiet_tokens
            .into_iter()
            .map(|((database, schema), quiet_token)| DynamicResumeLocation {
                location: WarehouseStateLocation { database, schema },
                quiet_token,
            })
            .collect(),
        failures,
    })
}

/// Where builds keep the pause table: the target's schema, unless it is `preserve` or unset.
fn pause_location<'py>(
    (discovered, target): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    adapter_name: &str,
    (config, adapter): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Option<WarehouseStateLocation>> {
    let py: Python<'py> = adapter.py();
    let vars: Bound<'py, PyAny> = build_effective_runtime_config(
        discovered,
        Some(target.clone()).filter(|target| !target.is_none()),
        None,
    )?
    .get_item(1)?;
    let settings: Bound<'py, PyAny> =
        selected_target_settings((discovered, target), adapter_name, (config, &vars, adapter))?
            .record(py)?;
    if field(&settings, "schema_preserved")?.extract::<bool>()? {
        return Ok(None);
    }
    let database: Option<String> = field(&settings, "database")?.extract()?;
    Ok(field(&settings, "schema")?
        .extract::<Option<String>>()?
        .map(|schema| WarehouseStateLocation { database, schema }))
}
