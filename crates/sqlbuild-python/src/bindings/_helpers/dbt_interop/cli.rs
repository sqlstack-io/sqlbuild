//! dbt argv building, executable choice, `dbt ls` output, `sqb dbt` argument routing and config.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use serde_json::{Map, Value};
use sqlbuild_core::dbt_cli::errors::DbtCliError;
use sqlbuild_core::dbt_cli::main::build_dbt_argv::build_dbt_argv;
use sqlbuild_core::dbt_cli::main::choose_dbt_config::choose_dbt_config;
use sqlbuild_core::dbt_cli::main::resolve_dbt_executable::resolve_dbt_executable as executable;
use sqlbuild_core::dbt_cli::main::route_dbt_interop_args::route_dbt_interop_args as route;
use sqlbuild_core::dbt_cli::models::{
    DbtArgvCommand, DbtCliOptionValues, DbtConfigChoice, DbtConfigSources, DbtInteropCommandKind,
    DbtParsedFlags, DbtRoutedArgs,
};
use sqlbuild_core::text::main::python_splitlines::python_splitlines;
use sqlbuild_core::text::main::python_strip::python_strip;
use sqlbuild_python_base::records::main::construct::new_error::new_error;

use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

const DBT_EXECUTABLE_ENV_VAR: &str = "DBT_EXECUTABLE";
const COMMAND_ARGV_OPTIONS: (&[&str], &[&str]) =
    (&["dbt_executable", "command", "options"], &["args"]);
const LS_ARGV_OPTIONS: (&[&str], &[&str]) = (
    &["dbt_executable", "options"],
    &["select", "exclude", "resource_types"],
);
const CONFIG_OPTIONS: (&[&str], &[&str]) = (
    &["project_root", "config", "overrides", "require_project_dir"],
    &["local_config"],
);

fn optional_text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.str()?.extract()?))
}

fn attribute_text(object: &Bound<'_, PyAny>, name: &str) -> PyResult<Option<String>> {
    optional_text(&native_field(object, name)?)
}

fn texts(values: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    values.try_iter()?.map(|value| value?.extract()).collect()
}

pub(crate) fn option_values(options: &Bound<'_, PyAny>) -> PyResult<DbtCliOptionValues> {
    Ok(DbtCliOptionValues {
        project_dir: attribute_text(options, "project_dir")?,
        profiles_dir: attribute_text(options, "profiles_dir")?,
        target: attribute_text(options, "target")?,
        target_path: attribute_text(options, "target_path")?,
        vars: attribute_text(options, "vars")?,
        state: attribute_text(options, "state")?,
        defer: native_field(options, "defer")?.is_truthy()?,
    })
}

pub(crate) fn argv<'py>(
    py: Python<'py>,
    (dbt_executable, options): (&str, &Bound<'py, PyAny>),
    command: DbtArgvCommand,
) -> PyResult<Bound<'py, PyTuple>> {
    PyTuple::new(
        py,
        build_dbt_argv(dbt_executable, &option_values(options)?, command),
    )
}

pub(crate) fn sequence(values: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if values.is_none() {
        return Ok(Vec::new());
    }
    texts(values)
}

/// The dbt executable, honouring a non-blank `DBT_EXECUTABLE` override.
#[pyfunction]
pub(crate) fn resolve_dbt_executable() -> String {
    let value: Option<String> =
        std::env::var_os(DBT_EXECUTABLE_ENV_VAR).map(|value| value.to_string_lossy().into_owned());
    executable(value.as_deref())
}

/// argv for `dbt compile`.
#[pyfunction]
#[pyo3(signature = (*, dbt_executable, options, full_refresh = false))]
fn build_dbt_compile_argv<'py>(
    py: Python<'py>,
    dbt_executable: &str,
    options: &Bound<'py, PyAny>,
    full_refresh: bool,
) -> PyResult<Bound<'py, PyTuple>> {
    argv(
        py,
        (dbt_executable, options),
        DbtArgvCommand::Compile { full_refresh },
    )
}

/// argv for `dbt deps`.
#[pyfunction]
#[pyo3(signature = (*, dbt_executable, options))]
fn build_dbt_deps_argv<'py>(
    py: Python<'py>,
    dbt_executable: &str,
    options: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    argv(py, (dbt_executable, options), DbtArgvCommand::Deps)
}

/// argv for `dbt debug`.
#[pyfunction]
#[pyo3(signature = (*, dbt_executable, options, args = None))]
fn build_dbt_debug_argv<'py>(
    py: Python<'py>,
    dbt_executable: &str,
    options: &Bound<'py, PyAny>,
    args: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let args: Vec<String> = args.map_or_else(|| Ok(Vec::new()), sequence)?;
    argv(
        py,
        (dbt_executable, options),
        DbtArgvCommand::Debug { args },
    )
}

/// argv for one executable dbt command with the common options.
#[pyfunction]
#[pyo3(signature = (**options))]
fn build_dbt_command_argv<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "build_dbt_command_argv", options, COMMAND_ARGV_OPTIONS)?;
    let command: DbtArgvCommand = DbtArgvCommand::Other {
        command: arguments.value("command")?.extract()?,
        args: sequence(&arguments.value("args")?)?,
    };
    let dbt_executable: String = arguments.value("dbt_executable")?.extract()?;
    argv(py, (&dbt_executable, &arguments.value("options")?), command)
}

/// argv for `dbt ls --output json`.
#[pyfunction]
#[pyo3(signature = (**options))]
fn build_dbt_ls_argv<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "build_dbt_ls_argv", options, LS_ARGV_OPTIONS)?;
    let command: DbtArgvCommand = DbtArgvCommand::Ls {
        select: sequence(&arguments.value("select")?)?,
        exclude: sequence(&arguments.value("exclude")?)?,
        resource_types: sequence(&arguments.value("resource_types")?)?,
    };
    let dbt_executable: String = arguments.value("dbt_executable")?.extract()?;
    argv(py, (&dbt_executable, &arguments.value("options")?), command)
}

fn json_text(payload: &Map<String, Value>, key: &str) -> Option<String> {
    payload.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn json_fqn(payload: &Map<String, Value>) -> Vec<String> {
    let Some(parts) = payload.get("fqn").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut names: Vec<String> = Vec::new();
    for part in parts {
        match part.as_str() {
            Some(name) if !name.is_empty() => names.push(name.to_owned()),
            _ => return Vec::new(),
        }
    }
    names
}

fn ls_node<'py>(
    py: Python<'py>,
    node_class: &Bound<'py, PyAny>,
    payload: &Map<String, Value>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let Some(unique_id) = json_text(payload, "unique_id").filter(|id| !id.is_empty()) else {
        return Ok(None);
    };
    let optional = |key: &str| -> Bound<'py, PyAny> {
        json_text(payload, key).map_or_else(
            || py.None().into_bound(py),
            |value| PyString::new(py, &value).into_any(),
        )
    };
    let node: Bound<'py, PyAny> = call_with(
        node_class,
        &[
            ("unique_id", PyString::new(py, &unique_id).into_any()),
            ("resource_type", optional("resource_type")),
            ("package_name", optional("package_name")),
            ("name", optional("name")),
            ("fqn", PyTuple::new(py, json_fqn(payload))?.into_any()),
            ("original_file_path", optional("original_file_path")),
        ],
    )?;
    Ok(Some(node))
}

/// Nodes from `dbt ls` JSON lines, skipping log lines, non-objects and nodes without an ID.
#[pyfunction]
#[pyo3(signature = (*, stdout))]
pub(crate) fn parse_dbt_ls_json_lines<'py>(
    py: Python<'py>,
    stdout: &str,
) -> PyResult<Bound<'py, PyTuple>> {
    ls_nodes(py, stdout)
}

/// The `DbtLsNode`s of `dbt ls --output json` lines.
pub(crate) fn ls_nodes<'py>(py: Python<'py>, stdout: &str) -> PyResult<Bound<'py, PyTuple>> {
    let node_class: Bound<'py, PyAny> = record_class(py, RecordId::DbtLsNode)?;
    let mut nodes: Vec<Bound<'py, PyAny>> = Vec::new();
    for line in python_splitlines(stdout) {
        let stripped: &str = python_strip(line);
        if !stripped.starts_with('{') {
            continue;
        }
        let Ok(Value::Object(payload)) = serde_json::from_str::<Value>(stripped) else {
            continue;
        };
        if let Some(node) = ls_node(py, &node_class, &payload)? {
            nodes.push(node);
        }
    }
    PyTuple::new(py, nodes)
}

pub(crate) fn raised(py: Python<'_>, class_name: ErrorId, error: DbtCliError) -> PyErr {
    new_error(py, class_name, &error.message, (error.code, error.help))
}

fn parsed_flags(parsed: &Bound<'_, PyAny>) -> PyResult<DbtParsedFlags> {
    let value = |name: &str| attribute_text(parsed, name);
    let flag = |name: &str| -> PyResult<bool> { native_field(parsed, name)?.is_truthy() };
    Ok(DbtParsedFlags {
        select: texts(&native_field(parsed, "select")?)?,
        exclude: texts(&native_field(parsed, "exclude")?)?,
        vars: value("vars")?,
        threads: value("threads")?,
        full_refresh: flag("full_refresh")?,
        event_time_start: value("event_time_start")?,
        event_time_end: value("event_time_end")?,
        project_dir: value("project_dir")?,
        profiles_dir: value("profiles_dir")?,
        profile: value("profile")?,
        target: value("target")?,
        target_path: value("target_path")?,
        state: value("state")?,
        indirect_selection: value("indirect_selection")?,
        defer: flag("defer")?,
        start_cursor_ts: value("start_cursor_ts")?,
        end_cursor_ts: value("end_cursor_ts")?,
        start_cursor_int: value("start_cursor_int")?,
        end_cursor_int: value("end_cursor_int")?,
        defer_to: value("defer_to")?,
        fail_fast: flag("fail_fast")?,
        hard_copy: flag("hard_copy")?,
        dbt_passthrough: texts(&native_field(parsed, "dbt_passthrough")?)?,
    })
}

/// Split parsed `sqb dbt <command>` flags into dbt and SQLBuild buckets.
#[pyfunction]
#[pyo3(signature = (*, command, parsed))]
pub(crate) fn route_dbt_interop_args<'py>(
    py: Python<'py>,
    command: &Bound<'py, PyAny>,
    parsed: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let command: String = command.str()?.extract()?;
    let flags: DbtParsedFlags = parsed_flags(parsed)?;
    let (kind, routed): (DbtInteropCommandKind, DbtRoutedArgs) =
        route(&command, &flags).map_err(|error| raised(py, ErrorId::DbtInteropArgument, error))?;
    let command: Bound<'py, PyAny> = enum_from_value(py, RecordId::DbtInteropCommand, kind.name())?;
    new_record_with(
        py,
        RecordId::DbtInteropRoutedArgs,
        &[
            ("command", command),
            ("select", PyTuple::new(py, flags.select)?.into_any()),
            ("exclude", PyTuple::new(py, flags.exclude)?.into_any()),
            ("dbt_args", PyTuple::new(py, routed.dbt_args)?.into_any()),
            (
                "sqlbuild_args",
                PyTuple::new(py, routed.sqlbuild_args)?.into_any(),
            ),
        ],
    )
}

/// `Path(raw).expanduser()` resolved, relative paths against the resolved project root.
pub(crate) fn resolved_path<'py>(
    project_root: &Bound<'py, PyAny>,
    raw_value: Option<String>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_root.py();
    let Some(raw_value) = raw_value else {
        return Ok(py.None().into_bound(py));
    };
    let root: Bound<'py, PyAny> = project_root
        .call_method0("expanduser")?
        .call_method0("resolve")?;
    let path: Bound<'py, PyAny> = imported(py, "pathlib", "Path")?
        .call1((raw_value,))?
        .call_method0("expanduser")?;
    if path.call_method0("is_absolute")?.is_truthy()? {
        return path.call_method0("resolve");
    }
    root.div(&path)?.call_method0("resolve")
}

fn config_sources(
    config: &Bound<'_, PyAny>,
    overrides: &Bound<'_, PyAny>,
    local_config: &Bound<'_, PyAny>,
) -> PyResult<DbtConfigSources> {
    let local_target: Option<String> = if local_config.is_none() {
        None
    } else {
        attribute_text(local_config, "target")?
    };
    Ok(DbtConfigSources {
        override_project_dir: attribute_text(overrides, "project_dir")?,
        config_project_dir: attribute_text(config, "project_dir")?,
        override_profiles_dir: attribute_text(overrides, "profiles_dir")?,
        config_profiles_dir: attribute_text(config, "profiles_dir")?,
        override_target_path: attribute_text(overrides, "target_path")?,
        config_target_path: attribute_text(config, "target_path")?,
        override_target: attribute_text(overrides, "target")?,
        local_target,
        config_target: attribute_text(config, "target")?,
    })
}

/// Resolve dbt config from CLI overrides, local config and project config.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn resolve_dbt_config<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "resolve_dbt_config", options, CONFIG_OPTIONS)?;
    let project_root: Bound<'py, PyAny> = arguments.value("project_root")?;
    let sources: DbtConfigSources = config_sources(
        &arguments.value("config")?,
        &arguments.value("overrides")?,
        &arguments.value("local_config")?,
    )?;
    let choice: DbtConfigChoice =
        choose_dbt_config(sources, arguments.flag("require_project_dir")?)
            .map_err(|error| raised(py, ErrorId::DbtInteropConfig, error))?;
    let target: Bound<'py, PyAny> = choice.target.map_or_else(
        || py.None().into_bound(py),
        |target| PyString::new(py, &target).into_any(),
    );
    new_record_with(
        py,
        RecordId::ResolvedDbtConfig,
        &[
            (
                "project_dir",
                resolved_path(&project_root, choice.project_dir)?,
            ),
            (
                "profiles_dir",
                resolved_path(&project_root, choice.profiles_dir)?,
            ),
            ("target", target),
            (
                "target_path",
                resolved_path(&project_root, choice.target_path)?,
            ),
        ],
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_dbt_executable, module)?)?;
    module.add_function(wrap_pyfunction!(build_dbt_compile_argv, module)?)?;
    module.add_function(wrap_pyfunction!(build_dbt_deps_argv, module)?)?;
    module.add_function(wrap_pyfunction!(build_dbt_debug_argv, module)?)?;
    module.add_function(wrap_pyfunction!(build_dbt_command_argv, module)?)?;
    module.add_function(wrap_pyfunction!(build_dbt_ls_argv, module)?)?;
    module.add_function(wrap_pyfunction!(parse_dbt_ls_json_lines, module)?)?;
    module.add_function(wrap_pyfunction!(route_dbt_interop_args, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_dbt_config, module)?)
}
