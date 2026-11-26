//! Validate loaded project and local configuration before discovery reads project files.

use crate::compile_driver::_helpers::connection_roles::target_connection_problem;
use crate::compile_driver::main::session_warehouse_support::session_warehouse_support;
use crate::compile_driver::models::ProjectConfiguration;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyBool, PyString, PyTuple};
use sqlbuild_config::project::main::unsupported_target_warehouse::unsupported_target_warehouse;
use sqlbuild_python_base::records::main::construct::new_error::new_error;

use crate::contracts::main::resolve_effective_adapter_name::resolve_effective_adapter_name;
use crate::contracts::main::resolve_target_config::resolve_target_config;
use crate::discovery::main::project::resolve_config_file_path::resolve_config_file_path;
use crate::discovery::main::project::validate_local_sql_analysis::validate_local_sql_analysis;
use pyo3::IntoPyObjectExt;
use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
const LOCAL_CONFIG_FILENAME: &str = "sqlbuild_local.toml";
const SQL_ANALYSIS_KEY: &str = "sql_analysis";
/// `WarehouseGroup` members in declaration order.
const WAREHOUSE_GROUPS: [&str; 3] = ["build", "query", "sync"];

fn project_config_error(py: Python<'_>, message: &str, help: Option<&str>) -> PyErr {
    new_error(py, ErrorId::ProjectConfig, message, (None, help))
}

/// The target names of one configuration's `targets` mapping.
fn target_names(config: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    native_field(config, "targets")?
        .try_iter()?
        .map(|name| name?.extract())
        .collect()
}

/// Reject a local SQL analysis opt-out the project forbids.
pub(crate) fn reject_local_sql_analysis_opt_out(
    config: &ProjectConfiguration<'_, '_>,
) -> PyResult<()> {
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
pub(crate) fn reject_unsupported_target_warehouses(
    config: &ProjectConfiguration<'_, '_>,
) -> PyResult<()> {
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
pub(crate) fn reject_invalid_target_connections(
    config: &ProjectConfiguration<'_, '_>,
) -> PyResult<()> {
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
        let error: PyErr = match resolved {
            Ok(resolved) => {
                let Some(problem) = target_connection_problem(config, (&name, &resolved))? else {
                    continue;
                };
                let config_path: Bound<'_, PyAny> =
                    config.project_dir.div(PROJECT_CONFIG_FILENAME)?;
                let message: String = format!("{} {problem}", config_path.str()?);
                return Err(project_config_error(py, &message, None));
            }
            Err(error) => error,
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
