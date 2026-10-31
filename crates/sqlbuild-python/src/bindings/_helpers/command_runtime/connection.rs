//! Connection config resolution for CLI commands.

use std::path::PathBuf;

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyString};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_config::command_connection::constants::{
    CONNECTION_SOURCE_KEY, DBT_PROFILE_CONNECTION_SOURCE, DBT_PROFILE_ROUTING_KEYS, DUCKDB_ADAPTER,
    DUCKDB_MEMORY_DATABASE, LOCAL_DATABASE_PATH_KEYS,
};
use sqlbuild_config::command_connection::main::missing_connection_error::missing_connection_error;
use sqlbuild_config::command_connection::main::snowflake_key_warning::snowflake_key_warning;
use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;
use sqlbuild_host::entries::models::ThirdPartyHost;

use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, imported, keywords};
use crate::bindings::_helpers::command_runtime::runtime_errors::raised;
use crate::bindings::_helpers::command_runtime::warehouses::{WarehouseInputs, apply_warehouse};
use crate::bindings::_helpers::compile_attachment::effective_config::build_effective_connection_config;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_effective_adapter_name;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_target_config;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_target_name;
use crate::bindings::_helpers::dbt_interop::profile_connection::resolve_raw_dbt_profile_connection;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::fields::field as native_field;

const ATTACH_KEY: &str = "attach";
const ATTACH_PATH_KEY: &str = "path";
const ATTACH_TYPE_KEY: &str = "type";
/// `ATTACH ... (TYPE ...)` values whose path is a local database file.
const LOCAL_ATTACH_TYPES: [&str; 2] = ["duckdb", "sqlite"];
const URL_SCHEME_SEPARATOR: &str = "://";
const MOTHERDUCK_PATH_PREFIX: &str = "md:";
/// A path naming the home directory, which DuckDB expands from `HOME`.
const HOME_PREFIX: &str = "~";
const HOME_DIRECTORY_PREFIX: &str = "~/";
const HOME_VARIABLE: &str = "HOME";

const CONNECTION_OPTIONS: (&[&str], &[&str]) = (
    &["raw_config", "project_dir", "adapter_name"],
    &["discovered_inputs", "cli_vars", "selected_target"],
);
const PROJECT_CONNECTION_OPTIONS: (&[&str], &[&str]) = (
    &["discovered_inputs", "project_dir"],
    &["selected_target", "cli_vars", "require_connection"],
);
const TARGET_CONNECTION_OPTIONS: (&[&str], &[&str]) = (
    &["discovered_inputs", "project_dir", "target_name"],
    &["cli_vars"],
);
const DBT_PROFILE_REQUIRES_PROJECT: &str =
    "connection.source = 'dbt_profile' requires project configuration";

fn path<'py>(py: Python<'py>, value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    imported(py, "pathlib", "Path")?.call1((value,))
}

fn warn(py: Python<'_>, message: &str) -> PyResult<()> {
    py.import("sys")?
        .getattr("stderr")?
        .call_method1("write", (format!("{message}\n"),))?;
    Ok(())
}

fn dbt_profile_error(py: Python<'_>) -> PyErr {
    match host::entry(py, ThirdPartyHost::DbtProfileError)
        .and_then(|class| class.call1((DBT_PROFILE_REQUIRES_PROJECT,)))
    {
        Ok(instance) => PyErr::from_value(instance),
        Err(error) => error,
    }
}

/// The config with a referenced dbt profile resolved and user overrides kept on top.
pub(crate) fn with_dbt_profile<'py>(
    raw_config: &Bound<'py, PyDict>,
    config: Bound<'py, PyDict>,
    (project_dir, adapter_name): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (discovered_inputs, cli_vars): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = raw_config.py();
    let profile: Bound<'py, PyAny> = resolve_raw_dbt_profile_connection(
        py,
        Some(keywords(
            py,
            &[
                ("raw_config", config.clone().into_any()),
                ("project_dir", project_dir.clone()),
                ("adapter_name", adapter_name.clone()),
                (
                    "project_config",
                    native_field(discovered_inputs, "project_config")?,
                ),
                ("cli_vars", cli_vars.clone()),
            ],
        )?),
    )?;
    if profile.is_none() {
        return Ok(config);
    }
    for warning in native_field(&profile, "warnings")?.try_iter()? {
        warn(py, &format!("Warning: {}", warning?.str()?))?;
    }
    let merged: Bound<'py, PyDict> = PyDict::new(py);
    merged.update(&native_field(&profile, "connection")?.cast_into()?)?;
    for (key, value) in raw_config.iter() {
        let routing: bool = key
            .extract::<String>()
            .is_ok_and(|key| DBT_PROFILE_ROUTING_KEYS.contains(&key.as_str()));
        if !routing {
            merged.set_item(key, value)?;
        }
    }
    Ok(merged)
}

/// Make a relative local database file path (DuckDB, SQLite) relative to the project directory.
pub(crate) fn anchor_local_database_path(
    config: &Bound<'_, PyDict>,
    (project_dir, adapter_name): (&Bound<'_, PyAny>, &str),
) -> PyResult<()> {
    let Some((_, path_key)) = LOCAL_DATABASE_PATH_KEYS
        .iter()
        .find(|(adapter, _)| *adapter == adapter_name)
    else {
        return Ok(());
    };
    if adapter_name == DUCKDB_ADAPTER {
        anchor_attach_paths(config, project_dir)?;
    }
    let Some(database) = config.get_item(*path_key)? else {
        return Ok(());
    };
    if let Some(anchored) = anchored_file_path(&database, project_dir)? {
        config.set_item(*path_key, anchored)?;
    }
    Ok(())
}

/// The file a local path names: `~` expanded, then `project_dir / path` when still relative.
fn anchored_file_path<'py>(
    value: &Bound<'py, PyAny>,
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    if !value.is_instance_of::<PyString>() {
        return Ok(None);
    }
    let text: String = value.extract()?;
    if text == DUCKDB_MEMORY_DATABASE {
        return Ok(None);
    }
    if let Some(rest) = home_relative_rest(&text) {
        return Ok(std::env::var_os(HOME_VARIABLE).map(|home| {
            let file: PathBuf = PathBuf::from(home).join(rest);
            PyString::new(py, &file.to_string_lossy()).into_any()
        }));
    }
    if path(py, value)?.call_method0("is_absolute")?.is_truthy()? {
        return Ok(None);
    }
    Ok(Some(path(py, project_dir)?.div(value)?.str()?.into_any()))
}

/// What follows `~` or `~/`, the home directory DuckDB expands a path from.
fn home_relative_rest(text: &str) -> Option<&str> {
    if text == HOME_PREFIX {
        return Some("");
    }
    text.strip_prefix(HOME_DIRECTORY_PREFIX)
}

/// Anchor each relative DuckDB or SQLite file an `attach` entry names; other paths are DSNs or URLs.
fn anchor_attach_paths(config: &Bound<'_, PyDict>, project_dir: &Bound<'_, PyAny>) -> PyResult<()> {
    let py: Python<'_> = config.py();
    let Some(entries) = config.get_item(ATTACH_KEY)? else {
        return Ok(());
    };
    let Ok(entries) = entries.try_iter() else {
        return Ok(());
    };
    let mut anchored: Vec<Bound<'_, PyAny>> = Vec::new();
    for entry in entries {
        let entry: Bound<'_, PyAny> = entry?;
        let Ok(mapping) = entry.cast::<PyDict>() else {
            anchored.push(entry);
            continue;
        };
        let copy: Bound<'_, PyDict> = mapping.copy()?;
        if let Some(path_value) = local_attach_file(mapping)?
            && let Some(file) = anchored_file_path(&path_value, project_dir)?
        {
            copy.set_item(ATTACH_PATH_KEY, file)?;
        }
        anchored.push(copy.into_any());
    }
    config.set_item(ATTACH_KEY, pyo3::types::PyList::new(py, anchored)?)
}

fn local_attach_file<'py>(mapping: &Bound<'py, PyDict>) -> PyResult<Option<Bound<'py, PyAny>>> {
    let local = match mapping.get_item(ATTACH_TYPE_KEY)? {
        None => true,
        Some(kind) => kind
            .extract::<String>()
            .is_ok_and(|kind| LOCAL_ATTACH_TYPES.contains(&kind.to_ascii_lowercase().as_str())),
    };
    Ok(mapping.get_item(ATTACH_PATH_KEY)?.filter(|value| {
        local
            && !value.extract::<String>().is_ok_and(|text| {
                text.contains(URL_SCHEME_SEPARATOR) || text.starts_with(MOTHERDUCK_PATH_PREFIX)
            })
    }))
}

/// Use the resolved adapter connection, including local attachments, not authored TOML guesses.
pub(crate) fn local_database_files(
    adapter: &str,
    config: &Bound<'_, PyDict>,
) -> PyResult<Vec<String>> {
    let Some((_, key)) = LOCAL_DATABASE_PATH_KEYS
        .iter()
        .find(|(name, _)| *name == adapter)
    else {
        return Ok(Vec::new());
    };
    let mut files: Vec<String> = Vec::new();
    if let Some(value) = config.get_item(*key)?
        && let Ok(file) = value.extract::<String>()
        && !file.is_empty()
        && file != DUCKDB_MEMORY_DATABASE
        && !file.contains(URL_SCHEME_SEPARATOR)
        && !file.starts_with(MOTHERDUCK_PATH_PREFIX)
    {
        files.push(file);
    }
    if adapter == DUCKDB_ADAPTER
        && let Some(entries) = config.get_item(ATTACH_KEY)?
        && let Ok(entries) = entries.try_iter()
    {
        for entry in entries {
            if let Ok(mapping) = entry?.cast_into::<PyDict>()
                && let Some(value) = local_attach_file(&mapping)?
                && let Ok(file) = value.extract::<String>()
                && file != DUCKDB_MEMORY_DATABASE
            {
                files.push(file);
            }
        }
    }
    Ok(files)
}

/// Resolve dbt profiles, relative DuckDB paths, and the command group's warehouse.
#[pyfunction]
#[pyo3(signature = (**options))]
fn resolve_connection_config<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "resolve_connection_config", options, CONNECTION_OPTIONS)?;
    connection_config(
        &arguments.value("raw_config")?.cast_into()?,
        (
            &arguments.value("project_dir")?,
            &arguments.value("adapter_name")?,
        ),
        (
            &arguments.value("discovered_inputs")?,
            &arguments.value("cli_vars")?,
        ),
        &arguments.value("selected_target")?,
    )
}

fn connection_config<'py>(
    raw_config: &Bound<'py, PyDict>,
    (project_dir, adapter_name): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (discovered_inputs, cli_vars): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    selected_target: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = raw_config.py();
    let mut config: Bound<'py, PyDict> = raw_config.copy()?;
    if !discovered_inputs.is_none() {
        config = with_dbt_profile(
            raw_config,
            config,
            (project_dir, adapter_name),
            (discovered_inputs, cli_vars),
        )?;
    } else if config
        .get_item(CONNECTION_SOURCE_KEY)?
        .map(|source| source.eq(DBT_PROFILE_CONNECTION_SOURCE))
        .transpose()?
        .unwrap_or(false)
    {
        return Err(dbt_profile_error(py));
    }
    let adapter: String = adapter_name.str()?.extract()?;
    let mut keys: Vec<String> = Vec::new();
    for key in config.keys() {
        if key.is_instance_of::<PyString>() {
            keys.push(key.extract()?);
        }
    }
    if let Some(warning) = snowflake_key_warning(&adapter, &keys) {
        warn(py, &warning)?;
    }
    anchor_local_database_path(&config, (project_dir, &adapter))?;
    apply_warehouse(
        config.as_any(),
        (adapter_name, project_dir),
        (!discovered_inputs.is_none()).then_some(WarehouseInputs {
            discovered_inputs,
            selected_target,
        }),
    )
}

/// Reject warehouse commands whose connection would come from no configuration at all.
fn require_configured_connection(
    discovered_inputs: &Bound<'_, PyAny>,
    project_dir: &Bound<'_, PyAny>,
    selected_target: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py: Python<'_> = project_dir.py();
    let project_config: Bound<'_, PyAny> = native_field(discovered_inputs, "project_config")?;
    let local_config: Bound<'_, PyAny> = native_field(discovered_inputs, "local_config")?;
    if native_field(&project_config, "connection")?.is_truthy()?
        || native_field(&local_config, "connection")?.is_truthy()?
    {
        return Ok(());
    }
    let target_name: Bound<'_, PyAny> = resolve_target_name(
        py,
        &project_config.clone(),
        &local_config.clone(),
        &selected_target.clone(),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    let project_file: String = path(py, project_dir)?
        .div(PROJECT_CONFIG_FILENAME)?
        .str()?
        .extract()?;
    if target_name.is_none() {
        return Err(raised(py, missing_connection_error(&project_file, None)));
    }
    let target_config: Bound<'_, PyAny> =
        resolve_target_config(py, &project_config, &local_config, &target_name.clone())?;
    if !native_field(&target_config, "connection_name")?.is_none()
        || native_field(&target_config, "connection")?.is_truthy()?
    {
        return Ok(());
    }
    let target_name: String = target_name.str()?.extract()?;
    Err(raised(
        py,
        missing_connection_error(&project_file, Some(&target_name)),
    ))
}

pub(crate) fn project_connection_config<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
    (project_dir, selected_target): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (cli_vars, require_connection): (&Bound<'py, PyAny>, bool),
) -> PyResult<Bound<'py, PyAny>> {
    if require_connection {
        require_configured_connection(discovered_inputs, project_dir, selected_target)?;
    }
    let raw_config: Bound<'py, PyDict> = build_effective_connection_config(
        &discovered_inputs.clone(),
        Some(selected_target.clone()).filter(|value| !value.is_none()),
        Some(cli_vars.clone()).filter(|value| !value.is_none()),
    )?
    .cast_into()?;
    let adapter_name: Bound<'py, PyAny> = resolve_effective_adapter_name(
        &native_field(discovered_inputs, "project_config")?,
        &native_field(discovered_inputs, "local_config")?,
    )?;
    connection_config(
        &raw_config,
        (project_dir, &adapter_name),
        (discovered_inputs, cli_vars),
        selected_target,
    )
}

/// Resolve the project connection; offline modes pass `require_connection=False`.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn resolve_project_connection_config<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "resolve_project_connection_config",
        options,
        PROJECT_CONNECTION_OPTIONS,
    )?;
    let require_connection: bool =
        !arguments.passed("require_connection")? || arguments.flag("require_connection")?;
    project_connection_config(
        &arguments.value("discovered_inputs")?,
        (
            &arguments.value("project_dir")?,
            &arguments.value("selected_target")?,
        ),
        (&arguments.value("cli_vars")?, require_connection),
    )
}

/// Resolve the effective connection config for one named target.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn resolve_target_connection_config<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "resolve_target_connection_config",
        options,
        TARGET_CONNECTION_OPTIONS,
    )?;
    project_connection_config(
        &arguments.value("discovered_inputs")?,
        (
            &arguments.value("project_dir")?,
            &arguments.value("target_name")?,
        ),
        (&arguments.value("cli_vars")?, true),
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_connection_config, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_project_connection_config, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_target_connection_config, module)?)
}
