//! Command-group warehouse selection for CLI warehouse connections.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyMapping, PyString};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_config::command_connection::constants::{
    CLI_WAREHOUSE_SOURCE, CONNECTION_DEFAULT_WAREHOUSE_SOURCE, CONNECTION_WAREHOUSE_KEY,
    CONNECTION_WAREHOUSE_SOURCE,
};
use sqlbuild_config::command_connection::main::authored_warehouse::authored_warehouse;
use sqlbuild_config::command_connection::main::check_cli_warehouse::check_cli_warehouse;
use sqlbuild_config::command_connection::main::check_expanded_warehouse::check_expanded_warehouse;
use sqlbuild_config::command_connection::main::unsupported_cli_warehouse::unsupported_cli_warehouse;
use sqlbuild_config::command_connection::models::AuthoredWarehouse;
use sqlbuild_core::text::main::python_strip::python_strip;

use sqlbuild_host::entries::models::AdapterHost;
use sqlbuild_model_config::templates::models::TemplateOptions;

use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, call_with};
use crate::bindings::_helpers::command_runtime::runtime_errors::{
    expected_resolution_error, raised,
};
use crate::bindings::_helpers::command_runtime::warehouse_scope::current_command_warehouse_scope;
use crate::bindings::_helpers::compile_attachment::effective_config::build_effective_runtime_config;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_effective_adapter_name;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_target_name;
use crate::bindings::_helpers::discovery::configuration::discover_project_configuration;
use crate::bindings::_helpers::model_config::template_data::expanded_template_data;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::ids::RecordId;

/// The `WarehouseGroup` whose warehouse is a target's `warehouses.build`; any other reads `query`.
const BUILD_GROUP: &str = "build";
const QUERY_GROUP: &str = "query";

const RESOLVE_OPTIONS: (&[&str], &[&str]) = (
    &[
        "discovered_inputs",
        "selected_target",
        "group",
        "cli_warehouse",
        "connection_config",
    ],
    &["cli_vars"],
);
const APPLY_OPTIONS: (&[&str], &[&str]) = (
    &[
        "config",
        "adapter_name",
        "project_dir",
        "discovered_inputs",
        "selected_target",
    ],
    &[],
);

/// The active command's warehouse group, `--warehouse` value and CLI vars.
pub(crate) struct CommandScope<'py> {
    pub(crate) group: Bound<'py, PyAny>,
    pub(crate) cli_warehouse: Option<String>,
    pub(crate) cli_vars: Bound<'py, PyAny>,
}

/// The project inputs and target a warehouse is resolved against.
#[derive(Clone, Copy)]
pub(crate) struct WarehouseInputs<'a, 'py> {
    pub(crate) discovered_inputs: &'a Bound<'py, PyAny>,
    pub(crate) selected_target: &'a Bound<'py, PyAny>,
}

fn none(py: Python<'_>) -> Bound<'_, PyAny> {
    py.None().into_bound(py)
}

fn optional_text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.extract()?))
}

pub(crate) fn command_scope(py: Python<'_>) -> PyResult<CommandScope<'_>> {
    let scope: Bound<'_, PyAny> = current_command_warehouse_scope(py)?;
    Ok(CommandScope {
        group: native_field(&scope, "group")?,
        cli_warehouse: optional_text(&native_field(&scope, "cli_warehouse")?)?,
        cli_vars: native_field(&scope, "cli_vars")?,
    })
}

/// `ResolvedWarehouse(warehouse=..., source=...)`.
fn resolved_warehouse<'py>(
    py: Python<'py>,
    warehouse: Option<&str>,
    source: &str,
) -> PyResult<Bound<'py, PyAny>> {
    new_record_with(
        py,
        RecordId::ResolvedWarehouse,
        &[
            (
                "warehouse",
                match warehouse {
                    Some(warehouse) => PyString::new(py, warehouse).into_any(),
                    None => none(py),
                },
            ),
            ("source", PyString::new(py, source).into_any()),
        ],
    )
}

fn group_value(target: &Bound<'_, PyAny>, group_name: &str) -> PyResult<Option<String>> {
    if target.is_none() {
        return Ok(None);
    }
    let attribute: &str = if group_name == BUILD_GROUP {
        BUILD_GROUP
    } else {
        QUERY_GROUP
    };
    optional_text(&native_field(
        &native_field(target, "warehouses")?,
        attribute,
    )?)
}

/// The active target's authored, unexpanded warehouse for one command group.
pub(crate) fn find_authored(
    inputs: WarehouseInputs<'_, '_>,
    group: &Bound<'_, PyAny>,
) -> PyResult<Option<AuthoredWarehouse>> {
    let group_name: String = native_field(group, "value")?.extract()?;
    authored_group(inputs, &group_name)
}

/// The active target's authored, unexpanded warehouse for the group named `group_name`.
fn authored_group<'py>(
    inputs: WarehouseInputs<'_, 'py>,
    group_name: &str,
) -> PyResult<Option<AuthoredWarehouse>> {
    let py: Python<'py> = inputs.discovered_inputs.py();
    let project_config: Bound<'py, PyAny> =
        native_field(inputs.discovered_inputs, "project_config")?;
    let local_config: Bound<'py, PyAny> = native_field(inputs.discovered_inputs, "local_config")?;
    let target_name: Bound<'py, PyAny> = resolve_target_name(
        py,
        &project_config.clone(),
        &local_config.clone(),
        &inputs.selected_target.clone(),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    if target_name.is_none() {
        return Ok(None);
    }
    let local_target: Bound<'py, PyAny> =
        native_field(&local_config, "targets")?.call_method1("get", (&target_name,))?;
    let project_target: Bound<'py, PyAny> =
        native_field(&project_config, "targets")?.call_method1("get", (&target_name,))?;
    let target_name: String = target_name.extract()?;
    Ok(authored_warehouse(
        (&target_name, group_name),
        group_value(&local_target, group_name)?,
        group_value(&project_target, group_name)?,
    ))
}

/// The group warehouse with `${...}` templates expanded, checked as an identifier.
fn expand_authored(
    authored: &AuthoredWarehouse,
    inputs: WarehouseInputs<'_, '_>,
    cli_vars: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let py: Python<'_> = inputs.discovered_inputs.py();
    let runtime: Bound<'_, PyAny> = build_effective_runtime_config(
        &inputs.discovered_inputs.clone(),
        Some(inputs.selected_target.clone()).filter(|value| !value.is_none()),
        Some(cli_vars.clone()).filter(|value| !value.is_none()),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    let expanded: Bound<'_, PyAny> = expanded_template_data(
        PyString::new(py, &authored.warehouse).as_any(),
        (runtime.get_item(1)?.downcast()?, &PyDict::new(py)),
        (
            TemplateOptions {
                allow_context: false,
                preserve_context_tokens: false,
                preserve_unknown_context: false,
            },
            &authored.context_label(),
        ),
    )?;
    let warehouse: String = expanded.str()?.extract()?;
    check_expanded_warehouse(authored, &warehouse).map_err(|error| raised(py, error))?;
    Ok(warehouse)
}

fn connection_warehouse(connection_config: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    let value: Bound<'_, PyAny> = mapping_get(connection_config, CONNECTION_WAREHOUSE_KEY)?;
    if !value.is_instance_of::<PyString>() {
        return Ok(None);
    }
    let text: String = value.extract()?;
    Ok((!python_strip(&text).is_empty()).then_some(text))
}

fn session_warehouse_support(
    py: Python<'_>,
    adapter_name: &Bound<'_, PyAny>,
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    call_with(
        &host::entry(py, AdapterHost::AdapterSupportsSessionWarehouse)?,
        &[
            ("adapter_name", adapter_name.clone()),
            ("project_dir", project_dir.clone()),
        ],
    )?
    .is_truthy()
}

fn checked_cli_warehouse(py: Python<'_>, cli_warehouse: &str) -> PyResult<()> {
    check_cli_warehouse(cli_warehouse).map_err(|error| raised(py, error))
}

fn unsupported(py: Python<'_>, cli_warehouse: &str, adapter_name: &Bound<'_, PyAny>) -> PyErr {
    match adapter_name.str().and_then(|name| name.extract::<String>()) {
        Ok(name) => raised(py, unsupported_cli_warehouse(cli_warehouse, &name)),
        Err(error) => error,
    }
}

/// One group's warehouse and its source: CLI, then target group, then connection, then default.
pub(crate) fn resolve_warehouse(
    inputs: Option<WarehouseInputs<'_, '_>>,
    (group, cli_warehouse): (&Bound<'_, PyAny>, Option<&str>),
    (connection_config, cli_vars): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<(Option<String>, String)> {
    let py: Python<'_> = group.py();
    if let Some(cli_warehouse) = cli_warehouse {
        checked_cli_warehouse(py, cli_warehouse)?;
        return Ok((
            Some(cli_warehouse.to_owned()),
            CLI_WAREHOUSE_SOURCE.to_owned(),
        ));
    }
    if let Some(inputs) = inputs
        && !group.is_none()
        && let Some(found) = find_authored(inputs, group)?
    {
        let warehouse: String = expand_authored(&found, inputs, cli_vars)?;
        return Ok((Some(warehouse), found.source()));
    }
    Ok(match connection_warehouse(connection_config)? {
        Some(warehouse) => (Some(warehouse), CONNECTION_WAREHOUSE_SOURCE.to_owned()),
        None => (None, CONNECTION_DEFAULT_WAREHOUSE_SOURCE.to_owned()),
    })
}

/// Every warehouse the target configures: its `build` and `query` group warehouses and its connection warehouse.
pub(crate) fn configured_warehouses(
    inputs: WarehouseInputs<'_, '_>,
    connection_config: &Bound<'_, PyAny>,
) -> PyResult<std::collections::BTreeSet<String>> {
    let none: Bound<'_, PyAny> = none(connection_config.py());
    let mut names: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for group_name in [BUILD_GROUP, QUERY_GROUP] {
        if let Some(authored) = authored_group(inputs, group_name)? {
            names.insert(expand_authored(&authored, inputs, &none)?);
        }
    }
    names.extend(connection_warehouse(connection_config)?);
    Ok(names)
}

fn optional_inputs<'a, 'py>(
    discovered_inputs: &'a Bound<'py, PyAny>,
    selected_target: &'a Bound<'py, PyAny>,
) -> Option<WarehouseInputs<'a, 'py>> {
    (!discovered_inputs.is_none()).then_some(WarehouseInputs {
        discovered_inputs,
        selected_target,
    })
}

/// Reject an empty, malformed, or unsupported `--warehouse` before the command runs.
#[pyfunction]
#[pyo3(signature = (*, cli_warehouse, project_dir))]
pub(crate) fn validate_cli_warehouse(
    py: Python<'_>,
    cli_warehouse: Option<String>,
    project_dir: Bound<'_, PyAny>,
) -> Result<(), PyErr> {
    let Some(cli_warehouse) = cli_warehouse else {
        return Ok(());
    };
    checked_cli_warehouse(py, &cli_warehouse)?;
    let discovered: Bound<'_, PyAny> = discover_project_configuration(py, &project_dir)?;
    let adapter_name: Bound<'_, PyAny> = resolve_effective_adapter_name(
        &native_field(&discovered, "project_config")?,
        &native_field(&discovered, "local_config")?,
    )?;
    if !session_warehouse_support(py, &adapter_name, &project_dir)? {
        return Err(unsupported(py, &cli_warehouse, &adapter_name));
    }
    Ok(())
}

/// Resolve one group's warehouse: CLI, then target group, then connection, then default.
#[pyfunction]
#[pyo3(signature = (**options))]
fn resolve_command_warehouse<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "resolve_command_warehouse", options, RESOLVE_OPTIONS)?;
    let discovered_inputs: Bound<'py, PyAny> = arguments.value("discovered_inputs")?;
    let selected_target: Bound<'py, PyAny> = arguments.value("selected_target")?;
    let cli_warehouse: Option<String> = optional_text(&arguments.value("cli_warehouse")?)?;
    let (warehouse, source) = resolve_warehouse(
        optional_inputs(&discovered_inputs, &selected_target),
        (&arguments.value("group")?, cli_warehouse.as_deref()),
        (
            &arguments.value("connection_config")?,
            &arguments.value("cli_vars")?,
        ),
    )?;
    resolved_warehouse(py, warehouse.as_deref(), &source)
}

/// The connection config with the active command group's warehouse applied.
pub(crate) fn apply_warehouse<'py>(
    config: &Bound<'py, PyAny>,
    (adapter_name, project_dir): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    inputs: Option<WarehouseInputs<'_, 'py>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = config.py();
    let scope: CommandScope<'py> = command_scope(py)?;
    let authored: bool = match inputs {
        Some(inputs) if !scope.group.is_none() => find_authored(inputs, &scope.group)?.is_some(),
        _ => false,
    };
    if scope.cli_warehouse.is_none() && !authored {
        return Ok(config.clone());
    }
    if !session_warehouse_support(py, adapter_name, project_dir)? {
        return match &scope.cli_warehouse {
            Some(cli_warehouse) => Err(unsupported(py, cli_warehouse, adapter_name)),
            None => Ok(config.clone()),
        };
    }
    let (warehouse, source) = resolve_warehouse(
        inputs,
        (&scope.group, scope.cli_warehouse.as_deref()),
        (config, &scope.cli_vars),
    )?;
    if source == CONNECTION_WAREHOUSE_SOURCE || source == CONNECTION_DEFAULT_WAREHOUSE_SOURCE {
        return Ok(config.clone());
    }
    let applied: Bound<'py, PyDict> = PyDict::new(py);
    applied.update(&config.cast::<PyMapping>()?.clone())?;
    applied.set_item(CONNECTION_WAREHOUSE_KEY, warehouse)?;
    Ok(applied.into_any())
}

/// Return connection config whose warehouse follows the active command's group.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn apply_command_warehouse<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "apply_command_warehouse", options, APPLY_OPTIONS)?;
    let discovered_inputs: Bound<'py, PyAny> = arguments.value("discovered_inputs")?;
    let selected_target: Bound<'py, PyAny> = arguments.value("selected_target")?;
    apply_warehouse(
        &arguments.value("config")?,
        (
            &arguments.value("adapter_name")?,
            &arguments.value("project_dir")?,
        ),
        optional_inputs(&discovered_inputs, &selected_target),
    )
}

/// Describe the warehouse an already resolved connection uses and where it came from.
#[pyfunction]
#[pyo3(signature = (*, discovered_inputs, selected_target, connection_config))]
pub(crate) fn describe_connection_warehouse<'py>(
    py: Python<'py>,
    discovered_inputs: Bound<'py, PyAny>,
    selected_target: Bound<'py, PyAny>,
    connection_config: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let scope: CommandScope<'py> = command_scope(py)?;
    let Some(warehouse) = connection_warehouse(&connection_config)? else {
        return resolved_warehouse(py, None, CONNECTION_DEFAULT_WAREHOUSE_SOURCE);
    };
    if scope.cli_warehouse.as_deref() == Some(warehouse.as_str()) {
        return resolved_warehouse(py, Some(&warehouse), CLI_WAREHOUSE_SOURCE);
    }
    let inputs: WarehouseInputs<'_, 'py> = WarehouseInputs {
        discovered_inputs: &discovered_inputs,
        selected_target: &selected_target,
    };
    let found: Option<AuthoredWarehouse> = if scope.group.is_none() {
        None
    } else {
        find_authored(inputs, &scope.group)?
    };
    if let Some(found) = found {
        let expands_to: bool = match expand_authored(&found, inputs, &scope.cli_vars) {
            Ok(expanded) => expanded == warehouse,
            Err(error) if expected_resolution_error(py, &error)? => false,
            Err(error) => return Err(error),
        };
        if expands_to {
            return resolved_warehouse(py, Some(&warehouse), &found.source());
        }
    }
    resolved_warehouse(py, Some(&warehouse), CONNECTION_WAREHOUSE_SOURCE)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(validate_cli_warehouse, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_command_warehouse, module)?)?;
    module.add_function(wrap_pyfunction!(apply_command_warehouse, module)?)?;
    module.add_function(wrap_pyfunction!(describe_connection_warehouse, module)?)
}
