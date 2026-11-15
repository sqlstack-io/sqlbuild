//! Validate loaded project and local configuration before discovery reads project files.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyString, PyTuple};
use sqlbuild_config::project::main::unsupported_target_warehouse::unsupported_target_warehouse;

use sqlbuild_host::entries::models::AdapterHost;

use sqlbuild_python_base::boundary::main::call_with::call_with;

use crate::bindings::_helpers::contracts::spec_resolution::{
    resolve_effective_adapter_name, resolve_target_config,
};
use crate::bindings::_helpers::discovery::project_config::{
    resolve_config_file_path, validate_local_sql_analysis,
};
use pyo3::IntoPyObjectExt;
use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
const LOCAL_CONFIG_FILENAME: &str = "sqlbuild_local.toml";
const SQL_ANALYSIS_KEY: &str = "sql_analysis";
/// `WarehouseGroup` members in declaration order.
const WAREHOUSE_GROUPS: [&str; 2] = ["build", "query"];

/// The loaded configuration of one project.
pub(crate) struct ProjectConfiguration<'a, 'py> {
    pub(crate) project_dir: &'a Bound<'py, PyAny>,
    pub(crate) project_config: &'a Bound<'py, PyAny>,
    pub(crate) local_config: &'a Bound<'py, PyAny>,
}

fn project_config_error(py: Python<'_>, message: &str, help: Option<&str>) -> PyErr {
    let raised = (|| -> PyResult<Bound<'_, PyAny>> {
        let keywords: Bound<'_, PyDict> = PyDict::new(py);
        keywords.set_item("help", help)?;
        error_class(py, ErrorId::ProjectConfig)?.call((message,), Some(&keywords))
    })();
    match raised {
        Ok(instance) => PyErr::from_value(instance),
        Err(failure) => failure,
    }
}

/// The target names of one configuration's `targets` mapping.
fn target_names(config: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    native_field(config, "targets")?
        .try_iter()?
        .map(|name| name?.extract())
        .collect()
}

/// Reject a local SQL analysis opt-out the project forbids.
fn reject_local_sql_analysis_opt_out(config: &ProjectConfiguration<'_, '_>) -> PyResult<()> {
    let py: Python<'_> = config.project_dir.py();
    let local_settings: Bound<'_, PyAny> = native_field(config.local_config, "settings")?;
    let flags: Bound<'_, PyTuple> = PyTuple::new(
        py,
        [
            native_field(config.project_config, "settings")?.getattr("require_sql_analysis")?,
            native_field(config.local_config, "setting_overrides")?
                .contains(SQL_ANALYSIS_KEY)
                .map(|overridden| PyBool::new(py, overridden).to_owned().into_any())?,
            native_field(&local_settings, SQL_ANALYSIS_KEY)?,
        ],
    )?;
    validate_local_sql_analysis(py, config.project_dir, flags.extract()?)
}

/// The first target, by name, that selects a command-group warehouse.
fn first_configured_group(config: &Bound<'_, PyAny>) -> PyResult<Option<(String, String, String)>> {
    let targets: Bound<'_, PyAny> = native_field(config, "targets")?;
    let mut names: Vec<String> = target_names(config)?;
    names.sort();
    for name in names {
        let warehouses: Bound<'_, PyAny> = native_field(&targets.get_item(&name)?, "warehouses")?;
        for group in WAREHOUSE_GROUPS {
            let value: Bound<'_, PyAny> = native_field(&warehouses, group)?;
            if !value.is_none() {
                return Ok(Some((name, group.to_owned(), value.extract()?)));
            }
        }
    }
    Ok(None)
}

/// Whether the adapter can switch warehouses per session, as the adapter host answers.
pub(crate) fn session_warehouse_support(
    py: Python<'_>,
    adapter_name: &Bound<'_, PyAny>,
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    call_with(
        &host_entry(py, AdapterHost::AdapterSupportsSessionWarehouse)?,
        &[
            ("adapter_name", adapter_name.clone()),
            ("project_dir", project_dir.clone()),
        ],
    )?
    .is_truthy()
}

fn reject_group(
    py: Python<'_>,
    (local, adapter_name): (bool, &Bound<'_, PyAny>),
    project_dir: &Bound<'_, PyAny>,
    (target, group, value): (String, String, String),
) -> PyErr {
    let raised = (|| -> PyResult<PyErr> {
        let file_path: String = resolve_config_file_path(py, project_dir, local)?
            .str()?
            .extract()?;
        let file_name: &str = if local {
            LOCAL_CONFIG_FILENAME
        } else {
            PROJECT_CONFIG_FILENAME
        };
        let (message, help) = unsupported_target_warehouse(
            (&file_path, file_name),
            &adapter_name.extract::<String>()?,
            (&target, &group, &value),
        );
        Ok(project_config_error(py, &message, Some(&help)))
    })();
    raised.unwrap_or_else(|failure| failure)
}

/// Reject command-group warehouses under adapters without a session warehouse.
fn reject_unsupported_target_warehouses(config: &ProjectConfiguration<'_, '_>) -> PyResult<()> {
    let py: Python<'_> = config.project_dir.py();
    if let Some(group) = first_configured_group(config.project_config)? {
        let adapter_name: Bound<'_, PyAny> = native_field(config.project_config, "adapter")?;
        if !session_warehouse_support(py, &adapter_name, config.project_dir)? {
            return Err(reject_group(
                py,
                (false, &adapter_name),
                config.project_dir,
                group,
            ));
        }
    }
    let Some(group) = first_configured_group(config.local_config)? else {
        return Ok(());
    };
    let adapter_name: Bound<'_, PyAny> = resolve_effective_adapter_name(
        &config.project_config.clone(),
        &config.local_config.clone(),
    )
    .and_then(|value| value.into_bound_py_any(py))?;
    if session_warehouse_support(py, &adapter_name, config.project_dir)? {
        return Ok(());
    }
    Err(reject_group(
        py,
        (true, &adapter_name),
        config.project_dir,
        group,
    ))
}

/// Resolve every configured target so invalid connections fail discovery.
fn reject_invalid_target_connections(config: &ProjectConfiguration<'_, '_>) -> PyResult<()> {
    let py: Python<'_> = config.project_dir.py();
    let mut names: Vec<String> = target_names(config.project_config)?;
    names.extend(target_names(config.local_config)?);
    names.sort();
    names.dedup();
    let spec_error: Bound<'_, PyAny> = error_class(py, ErrorId::SpecConfig)?;
    for name in names {
        let resolved: PyResult<Bound<'_, PyAny>> = resolve_target_config(
            py,
            config.project_config,
            config.local_config,
            PyString::new(py, &name).as_any(),
        );
        let Err(error) = resolved else {
            continue;
        };
        if !error.is_instance(py, &spec_error) {
            return Err(error);
        }
        let config_path: Bound<'_, PyAny> = config.project_dir.div(PROJECT_CONFIG_FILENAME)?;
        let message: String = format!("{} {}", config_path.str()?, error.value(py).str()?);
        let raised: PyErr = project_config_error(py, &message, None);
        raised.set_cause(py, Some(error));
        return Err(raised);
    }
    Ok(())
}

/// Validate the SQL analysis policy, target warehouses and target connections, in that order.
pub(crate) fn reject_invalid_project_configuration(
    config: &ProjectConfiguration<'_, '_>,
) -> PyResult<()> {
    reject_local_sql_analysis_opt_out(config)?;
    reject_invalid_connection_configuration(config)
}

/// Validate the target warehouses and target connections a warehouse connection depends on.
pub(crate) fn reject_invalid_connection_configuration(
    config: &ProjectConfiguration<'_, '_>,
) -> PyResult<()> {
    reject_unsupported_target_warehouses(config)?;
    reject_invalid_target_connections(config)
}
