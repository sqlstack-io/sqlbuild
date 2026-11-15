//! `sqb dbt` execution flags, routed dbt config overrides, vars, plan options and the adapter.

use pyo3::exceptions::PySystemExit;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_cli::argv::main::parse_dbt_execution_arguments::parse_dbt_execution_arguments;
use sqlbuild_cli::argv::models::{ArgvSettings, DbtExecutionFailure, DbtExecutionOutcome, Stream};
use sqlbuild_core::dbt_cli::errors::DbtCliError;
use sqlbuild_core::dbt_cli::main::dbt_flag_value::dbt_flag_value;
use sqlbuild_core::dbt_cli::models::DbtParsedFlags;
use sqlbuild_host::entries::models::AdapterHost;

use crate::bindings::_helpers::dbt_interop::cli::resolve_dbt_config;
use crate::bindings::_helpers::dbt_interop::cli::{raised, resolved_path};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

const ARGUMENT_ERROR: ErrorId = ErrorId::DbtInteropArgument;
const CONFIG_ERROR: ErrorId = ErrorId::DbtInteropConfig;
const ARGUMENT_CODE: &str = "C238";
const UNRECOGNIZED_HELP: &str = "SQLBuild flags go before `--`; pass raw dbt flags after `--`.";
const PARSER_HELP: &str = "Run `-h` to list supported flags; pass raw dbt flags after `--`.";
const EXECUTION_COMMANDS: [&str; 3] = ["plan", "run", "build"];
const DISPLAY_FLAGS: [&str; 3] = ["--json", "--verbose", "-v"];
const PLAN_COMMAND: &str = "plan";
const JSON_FLAG: &str = "--json";
const JSON_HELP: &str =
    "Use sqb dbt plan --json for a machine-readable preview of the combined work.";
const DEFER_FLAG: &str = "--defer";
const VARS_FLAG: &str = "--vars";
const STATE_FLAG: &str = "--state";
const MANIFEST_FILENAME: &str = "manifest.json";
const DEFAULT_TARGET_DIR: &str = "target";

fn none(py: Python<'_>) -> Bound<'_, PyAny> {
    py.None().into_bound(py)
}

fn optional_text<'py>(py: Python<'py>, value: Option<String>) -> Bound<'py, PyAny> {
    value.map_or_else(|| none(py), |text| PyString::new(py, &text).into_any())
}

fn flag(py: Python<'_>, value: bool) -> Bound<'_, PyAny> {
    PyBool::new(py, value).to_owned().into_any()
}

fn argument_error(py: Python<'_>, message: String, help: &'static str) -> PyErr {
    raised(
        py,
        ARGUMENT_ERROR,
        DbtCliError {
            message,
            code: Some(ARGUMENT_CODE),
            help: Some(help),
        },
    )
}

fn write_outputs(py: Python<'_>, outcome: &DbtExecutionOutcome) -> PyResult<()> {
    let sys: Bound<'_, PyModule> = py.import("sys")?;
    for (stream, text) in &outcome.writes {
        let name: &str = match stream {
            Stream::Stdout => "stdout",
            Stream::Stderr => "stderr",
        };
        native_field(&sys, name)?.call_method1("write", (text,))?;
    }
    Ok(())
}

/// Parse raw `sqb dbt <command>` tokens into declared flags and a `--` tail.
pub(crate) fn parsed_flags(
    py: Python<'_>,
    command: &str,
    args: &[String],
) -> PyResult<DbtParsedFlags> {
    let columns: i64 = py
        .import("shutil")?
        .call_method0("get_terminal_size")?
        .getattr("columns")?
        .extract()?;
    let settings: ArgvSettings = ArgvSettings {
        columns,
        use_color: false,
    };
    let outcome: DbtExecutionOutcome = parse_dbt_execution_arguments(command, args, settings);
    write_outputs(py, &outcome)?;
    match outcome.parsed {
        Ok(flags) => Ok(flags),
        Err(DbtExecutionFailure::Exit(code)) => Err(PySystemExit::new_err((code,))),
        Err(DbtExecutionFailure::Argument(message)) => {
            Err(argument_error(py, message, PARSER_HELP))
        }
        Err(DbtExecutionFailure::Unrecognized(unknown)) => Err(argument_error(
            py,
            format!(
                "unrecognized option(s) for sqb dbt {command}: {}",
                unknown.join(" ")
            ),
            UNRECOGNIZED_HELP,
        )),
    }
}

fn parsed_args_model<'py>(py: Python<'py>, flags: DbtParsedFlags) -> PyResult<Bound<'py, PyAny>> {
    new_record_with(
        py,
        RecordId::DbtInteropParsedArgs,
        &[
            ("select", PyTuple::new(py, flags.select)?.into_any()),
            ("exclude", PyTuple::new(py, flags.exclude)?.into_any()),
            ("vars", optional_text(py, flags.vars)),
            ("threads", optional_text(py, flags.threads)),
            ("full_refresh", flag(py, flags.full_refresh)),
            (
                "event_time_start",
                optional_text(py, flags.event_time_start),
            ),
            ("event_time_end", optional_text(py, flags.event_time_end)),
            ("project_dir", optional_text(py, flags.project_dir)),
            ("profiles_dir", optional_text(py, flags.profiles_dir)),
            ("profile", optional_text(py, flags.profile)),
            ("target", optional_text(py, flags.target)),
            ("target_path", optional_text(py, flags.target_path)),
            ("state", optional_text(py, flags.state)),
            (
                "indirect_selection",
                optional_text(py, flags.indirect_selection),
            ),
            ("defer", flag(py, flags.defer)),
            ("defer_to", optional_text(py, flags.defer_to)),
            ("start_cursor_ts", optional_text(py, flags.start_cursor_ts)),
            ("end_cursor_ts", optional_text(py, flags.end_cursor_ts)),
            (
                "start_cursor_int",
                optional_text(py, flags.start_cursor_int),
            ),
            ("end_cursor_int", optional_text(py, flags.end_cursor_int)),
            ("fail_fast", flag(py, flags.fail_fast)),
            ("hard_copy", flag(py, flags.hard_copy)),
            (
                "dbt_passthrough",
                PyTuple::new(py, flags.dbt_passthrough)?.into_any(),
            ),
        ],
    )
}

fn texts(values: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    values.try_iter()?.map(|value| value?.extract()).collect()
}

/// Parse raw `sqb dbt <command>` tokens into declared flags and a `--` tail.
#[pyfunction]
#[pyo3(signature = (*, command, args))]
pub(crate) fn parse_dbt_execution_args<'py>(
    command: &Bound<'py, PyAny>,
    args: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = command.py();
    let name: String = command.str()?.extract()?;
    parsed_args_model(py, parsed_flags(py, &name, &texts(args)?)?)
}

/// Parse execution-command args early so `-h` and typos resolve before setup.
pub(crate) fn validate_dbt_execution_args(
    command: &Bound<'_, PyAny>,
    args: &Bound<'_, PyAny>,
) -> Result<(), PyErr> {
    let name: String = command.str()?.extract()?;
    if !EXECUTION_COMMANDS.contains(&name.as_str()) {
        return Ok(());
    }
    let given: Vec<String> = texts(args)?;
    if name != PLAN_COMMAND && given.iter().any(|arg| arg == JSON_FLAG) {
        return Err(argument_error(
            command.py(),
            format!("sqb dbt {name} does not support --json"),
            JSON_HELP,
        ));
    }
    let display_free: Vec<String> = given
        .into_iter()
        .filter(|arg| !DISPLAY_FLAGS.contains(&arg.as_str()))
        .collect();
    parsed_flags(command.py(), &name, &display_free).map(|_| ())
}

/// dbt project, profile, target and target-path overrides from routed dbt args.
pub(crate) fn config_overrides<'py>(
    py: Python<'py>,
    args: &[String],
) -> PyResult<Bound<'py, PyAny>> {
    let value = |name: &str| optional_text(py, dbt_flag_value(args, name));
    new_record_with(
        py,
        RecordId::DbtCliConfigOverrides,
        &[
            ("project_dir", value("--project-dir")),
            ("profiles_dir", value("--profiles-dir")),
            ("target", value("--target")),
            ("target_path", value("--target-path")),
        ],
    )
}

fn config_error(py: Python<'_>, message: &str) -> PyErr {
    raised(
        py,
        CONFIG_ERROR,
        DbtCliError {
            message: message.to_owned(),
            code: None,
            help: None,
        },
    )
}

fn cli_vars<'py>(py: Python<'py>, raw: Option<String>) -> PyResult<Bound<'py, PyDict>> {
    let result: Bound<'py, PyDict> = PyDict::new(py);
    let Some(raw) = raw else {
        return Ok(result);
    };
    let parsed: Bound<'py, PyAny> = py.import("yaml")?.call_method1("safe_load", (raw,))?;
    if parsed.is_none() {
        return Ok(result);
    }
    let Ok(parsed) = parsed.cast_into::<PyDict>() else {
        return Err(config_error(py, "dbt --vars must be a mapping"));
    };
    for (key, value) in parsed.iter() {
        if !key.is_instance_of::<PyString>() {
            return Err(config_error(py, "dbt --vars keys must be strings"));
        }
        result.set_item(key, value)?;
    }
    Ok(result)
}

/// Merged dbt vars: project config, then local config, then `--vars`.
pub(crate) fn vars_mapping<'py>(
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
    args: &[String],
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = project_config.py();
    let values: Bound<'py, PyDict> = PyDict::new(py);
    values.call_method1("update", (native_field(project_config, "vars")?,))?;
    values.call_method1("update", (native_field(local_config, "vars")?,))?;
    values.update(cli_vars(py, dbt_flag_value(args, VARS_FLAG))?.as_mapping())?;
    Ok(values)
}

/// Return merged dbt vars as a structured mapping.
#[pyfunction]
#[pyo3(signature = (*, project_config, local_config, dbt_args))]
fn resolve_dbt_vars_mapping<'py>(
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
    dbt_args: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    vars_mapping(project_config, local_config, &texts(dbt_args)?)
}

/// Resolved dbt CLI options for `sqb dbt plan|run|build`.
pub(crate) fn plan_options<'py>(
    project_dir: &Bound<'py, PyAny>,
    discovered_inputs: &Bound<'py, PyAny>,
    args: &[String],
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    let project_config: Bound<'py, PyAny> =
        native_field(discovered_inputs, "project_config")?.getattr("dbt")?;
    let local_config: Bound<'py, PyAny> =
        native_field(discovered_inputs, "local_config")?.getattr("dbt")?;
    let resolved: Bound<'py, PyAny> = resolve_dbt_config(
        py,
        Some(keywords(
            py,
            &[
                ("project_root", project_dir.clone()),
                ("config", project_config.clone()),
                ("local_config", local_config.clone()),
                ("overrides", config_overrides(py, args)?),
                ("require_project_dir", flag(py, true)),
            ],
        )?),
    )?;
    let values: Bound<'py, PyDict> = vars_mapping(&project_config, &local_config, args)?;
    let vars: Bound<'py, PyAny> = if values.is_empty() {
        none(py)
    } else {
        call_with(
            &py.import("json")?.getattr("dumps")?,
            &[("obj", values.into_any()), ("sort_keys", flag(py, true))],
        )?
    };
    new_record_with(
        py,
        RecordId::DbtCliOptions,
        &[
            ("project_dir", native_field(&resolved, "project_dir")?),
            ("profiles_dir", native_field(&resolved, "profiles_dir")?),
            ("target", native_field(&resolved, "target")?),
            ("target_path", native_field(&resolved, "target_path")?),
            ("vars", vars),
            (
                "state",
                resolved_path(project_dir, dbt_flag_value(args, STATE_FLAG))?,
            ),
            ("defer", flag(py, args.iter().any(|arg| arg == DEFER_FLAG))),
        ],
    )
}

/// The manifest path `dbt compile` writes for these options.
pub(crate) fn manifest_path<'py>(options: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = options.py();
    let project_dir: Bound<'py, PyAny> = native_field(options, "project_dir")?;
    if project_dir.is_none() {
        return Err(config_error(py, "dbt project directory is not configured"));
    }
    let mut target_path: Bound<'py, PyAny> = native_field(options, "target_path")?;
    if !target_path.is_truthy()? {
        target_path = project_dir.div(DEFAULT_TARGET_DIR)?;
    }
    target_path.div(MANIFEST_FILENAME)
}

/// A project adapter of this name, else the built-in one, instantiated for interop planning.
pub(crate) fn interop_adapter<'py>(
    adapter_name: &Bound<'py, PyAny>,
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = adapter_name.py();
    let builtin: Bound<'py, PyDict> = host_entry(py, AdapterHost::BuiltinAdapterClasses)?
        .call0()?
        .cast_into()?;
    let mut adapter_class: Option<Bound<'py, PyAny>> = None;
    if !project_dir.is_none() {
        let reserved: Bound<'py, PyAny> = py
            .import("builtins")?
            .getattr("frozenset")?
            .call1((builtin.clone(),))?;
        let local: Bound<'py, PyDict> = call_with(
            &host_entry(py, AdapterHost::DiscoverProjectAdapters)?,
            &[
                ("project_dir", project_dir.clone()),
                ("reserved_names", reserved),
            ],
        )?
        .cast_into()?;
        adapter_class = local.get_item(adapter_name)?;
    }
    if adapter_class.is_none() {
        adapter_class = builtin.get_item(adapter_name)?;
    }
    let Some(adapter_class) = adapter_class else {
        let mut available: Vec<String> = Vec::new();
        for name in builtin.keys() {
            available.push(name.str()?.extract()?);
        }
        available.sort();
        return Err(config_error(
            py,
            &format!(
                "unknown adapter '{}'. Available built-in adapters: {}.",
                adapter_name.str()?,
                available.join(", ")
            ),
        ));
    };
    adapter_class.call0()
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(parse_dbt_execution_args, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_dbt_vars_mapping, module)?)
}
