//! `sqb debug`: report the runtime, resolved configuration and connection health.

use std::path::PathBuf;

use pyo3::exceptions::PyException;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyFloat, PyInt, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::debug_report::main::debug_config_files::debug_config_files;
use sqlbuild_core::debug_report::main::debug_configuration_lines::debug_configuration_lines;
use sqlbuild_core::debug_report::main::debug_connection_lines::debug_connection_lines;
use sqlbuild_core::debug_report::main::debug_failure::debug_failure;
use sqlbuild_core::debug_report::main::debug_provider_lines::debug_provider_lines;
use sqlbuild_core::debug_report::main::debug_runtime_lines::debug_runtime_lines;
use sqlbuild_core::debug_report::main::format_debug_json::format_debug_json;
use sqlbuild_core::debug_report::main::format_debug_text::format_debug_text;
use sqlbuild_core::debug_report::models::{
    ConnectionCheck, ConnectionValue, DebugConfigFiles, DebugProjectFacts, DebugProvider,
    DebugReport, DebugRuntime, WarehouseCheck,
};

use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::boundary::python_calls::{
    KeywordArguments, call_with, imported, within,
};
use crate::bindings::_helpers::cli::failure_cause::{FailureCause, report_failure_cause};
use crate::bindings::_helpers::command_runtime::adapters::resolve_cli_adapter;
use crate::bindings::_helpers::command_runtime::connection::{
    local_database_files, project_connection_config,
};
use crate::bindings::_helpers::command_runtime::runtime_errors::{
    expected_resolution_error, resolution_message,
};
use crate::bindings::_helpers::command_runtime::warehouse_scope::command_warehouse_scope;
use crate::bindings::_helpers::command_runtime::warehouses::{
    CommandScope, WarehouseInputs, apply_warehouse, command_scope, find_authored, resolve_warehouse,
};
use crate::bindings::_helpers::compile_attachment::target::build_effective_target_namespace;
use crate::bindings::_helpers::compile_driver::discovery::discover_project_inputs;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_effective_adapter_name;
use crate::bindings::_helpers::native_host::host::redirect_stdout_to_stderr;
use crate::bindings::_helpers::presentation_text::terminal::stdout_supports_color;
use crate::bindings::_helpers::records::construct::record_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;

const DEBUG_OPTIONS: (&[&str], &[&str]) = (
    &[],
    &[
        "project_dir",
        "no_color",
        "no_connection",
        "selected_target",
        "json_output",
    ],
);

/// The Python objects and options one debug invocation resolves against.
struct DebugContext<'py> {
    project_dir: Bound<'py, PyAny>,
    discovered_inputs: Bound<'py, PyAny>,
    selected_target: Bound<'py, PyAny>,
    adapter: Bound<'py, PyAny>,
}

fn runtime(py: Python<'_>) -> PyResult<DebugRuntime> {
    let platform: Bound<'_, PyModule> = py.import("platform")?;
    let sqlbuild_version: Option<String> =
        match imported(py, "importlib.metadata", "version")?.call1(("sqlbuild",)) {
            Ok(version) => Some(version.str()?.extract()?),
            Err(error)
                if error.is_instance(
                    py,
                    &imported(py, "importlib.metadata", "PackageNotFoundError")?,
                ) =>
            {
                None
            }
            Err(error) => return Err(error),
        };
    Ok(DebugRuntime {
        sqlbuild_version,
        python_version: platform.call_method0("python_version")?.str()?.extract()?,
        python_path: py.import("sys")?.getattr("executable")?.str()?.extract()?,
        os_info: platform.call_method0("platform")?.str()?.extract()?,
    })
}

/// A non-empty string value, as `_display_target_value` accepts it.
fn target_value(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if !value.is_instance_of::<PyString>() {
        return Ok(None);
    }
    let text: String = value.extract()?;
    Ok((!text.is_empty()).then_some(text))
}

/// `str(path)` of a joined path, normalised as `pathlib` normalises it.
fn path_text(py: Python<'_>, path: &PathBuf) -> PyResult<String> {
    imported(py, "pathlib", "Path")?
        .call1((path,))?
        .str()?
        .extract()
}

fn optional_string(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.str()?.extract()?))
}

fn warehouse_checks(
    context: &DebugContext<'_>,
    (command_scope, base_connection): (&CommandScope<'_>, &Bound<'_, PyAny>),
) -> PyResult<Vec<WarehouseCheck>> {
    let py: Python<'_> = context.adapter.py();
    let supports_session_warehouse: bool =
        native_field(&context.adapter, "supports_session_warehouse")?.is_truthy()?;
    let adapter_name: String = native_field(&context.adapter, "adapter_name")?
        .str()?
        .extract()?;
    let inputs: WarehouseInputs<'_, '_> = WarehouseInputs {
        discovered_inputs: &context.discovered_inputs,
        selected_target: &context.selected_target,
    };
    let mut checks: Vec<WarehouseCheck> = Vec::new();
    for group in record_class(py, RecordId::WarehouseGroup)?.try_iter()? {
        let group: Bound<'_, PyAny> = group?;
        let group_name: String = native_field(&group, "value")?.str()?.extract()?;
        if !supports_session_warehouse {
            if let Some(found) = find_authored(inputs, &group)? {
                checks.push(WarehouseCheck::NotUsed {
                    group: group_name,
                    adapter_name: adapter_name.clone(),
                    source: found.authored.source(),
                });
            }
            continue;
        }
        let resolved: PyResult<(Option<String>, String)> = resolve_warehouse(
            Some(inputs),
            (&group, command_scope.cli_warehouse.as_deref()),
            (base_connection, &command_scope.cli_vars),
        );
        checks.push(match resolved {
            Ok((warehouse, source)) => WarehouseCheck::Resolved {
                group: group_name,
                warehouse: warehouse.filter(|warehouse| !warehouse.is_empty()),
                source,
            },
            Err(error) if expected_resolution_error(py, &error)? => WarehouseCheck::Unresolved {
                group: group_name,
                error: resolution_message(py, &error)?,
            },
            Err(error) => return Err(error),
        });
    }
    Ok(checks)
}

fn providers(discovered_inputs: &Bound<'_, PyAny>) -> PyResult<Vec<DebugProvider>> {
    let mut providers: Vec<DebugProvider> = Vec::new();
    for provider in native_field(discovered_inputs, "providers")?.try_iter()? {
        let provider: Bound<'_, PyAny> = provider?;
        providers.push(DebugProvider {
            name: native_field(&provider, "name")?.str()?.extract()?,
            relative_path: native_field(&provider, "relative_path")?.str()?.extract()?,
            class_name: native_field(&provider, "provider_class")?
                .getattr("__name__")?
                .str()?
                .extract()?,
        });
    }
    Ok(providers)
}

fn connection_settings(config: &Bound<'_, PyDict>) -> PyResult<Vec<(String, ConnectionValue)>> {
    let mut settings: Vec<(String, ConnectionValue)> = Vec::new();
    for (key, value) in config.iter() {
        let shown: ConnectionValue = if value.is_none() {
            ConnectionValue::Null
        } else if value.is_instance_of::<PyBool>()
            || value.is_instance_of::<PyInt>()
            || value.is_instance_of::<PyFloat>()
        {
            ConnectionValue::Scalar(value.str()?.extract()?)
        } else {
            ConnectionValue::Text(value.str()?.extract()?)
        };
        settings.push((key.extract()?, shown));
    }
    Ok(settings)
}

/// The message of an ordinary exception; anything else, such as an interrupt, propagates.
fn check_failure(py: Python<'_>, error: PyErr) -> PyResult<String> {
    if !error.is_instance_of::<PyException>(py) {
        return Err(error);
    }
    error.value(py).str()?.extract()
}

fn connection_check(
    adapter: &Bound<'_, PyAny>,
    connection_config: &Bound<'_, PyDict>,
) -> PyResult<ConnectionCheck> {
    let py: Python<'_> = adapter.py();
    let connection: Bound<'_, PyAny> = match adapter.call_method1("connect", (connection_config,)) {
        Ok(connection) => connection,
        Err(error) => return Ok(ConnectionCheck::ConnectFailed(check_failure(py, error)?)),
    };
    let queried: PyResult<Bound<'_, PyAny>> = call_with(
        &native_field(adapter, "query")?,
        &[
            ("connection", connection.clone()),
            ("sql", PyString::new(py, "SELECT 1").into_any()),
            ("limit", py.None().into_bound(py)),
        ],
    );
    let query_error: Option<String> = match queried {
        Ok(_) => None,
        Err(error) if error.is_instance_of::<PyException>(py) => {
            adapter.call_method1("close", (&connection,))?;
            Some(error.value(py).str()?.extract()?)
        }
        Err(error) => {
            adapter.call_method1("close", (&connection,))?;
            return Err(error);
        }
    };
    if query_error.is_none() {
        adapter.call_method1("close", (&connection,))?;
    }
    Ok(ConnectionCheck::Connected { query_error })
}

/// The effective connection: the base connection with the command's warehouse applied.
fn effective_connection<'py>(
    context: &DebugContext<'py>,
    (adapter_name, base_connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = adapter_name.py();
    let applied: PyResult<Bound<'py, PyAny>> = apply_warehouse(
        base_connection,
        (adapter_name, &context.project_dir),
        Some(WarehouseInputs {
            discovered_inputs: &context.discovered_inputs,
            selected_target: &context.selected_target,
        }),
    );
    match applied {
        Err(error) if expected_resolution_error(py, &error)? => Ok(base_connection.clone()),
        applied => applied,
    }
}

fn build_report(
    py: Python<'_>,
    project_dir: &PathBuf,
    (selected_target, check_connection): (Bound<'_, PyAny>, bool),
) -> PyResult<DebugReport> {
    let project_dir_object: Bound<'_, PyAny> =
        imported(py, "pathlib", "Path")?.call1((project_dir,))?;
    let discovered_inputs: Bound<'_, PyAny> = discover_project_inputs(
        py,
        Some(keywords(
            py,
            &[("project_dir", project_dir_object.clone())],
        )?),
    )?;
    let files: DebugConfigFiles = debug_config_files(project_dir);
    let adapter_name: Bound<'_, PyAny> = resolve_effective_adapter_name(
        &native_field(&discovered_inputs, "project_config")?,
        &native_field(&discovered_inputs, "local_config")?,
    )?;
    let adapter: Bound<'_, PyAny> = resolve_cli_adapter(&adapter_name, &project_dir_object)?;
    let context: DebugContext<'_> = DebugContext {
        project_dir: project_dir_object,
        discovered_inputs,
        selected_target,
        adapter,
    };
    let command_scope: CommandScope<'_> = command_scope(py)?;
    let none: Bound<'_, PyAny> = py.None().into_bound(py);
    let unscoped: Bound<'_, PyAny> =
        Bound::new(py, command_warehouse_scope(&none, &none, None)?)?.into_any();
    let base_connection: Bound<'_, PyAny> = within(&unscoped, |_| {
        project_connection_config(
            &context.discovered_inputs,
            (&context.project_dir, &context.selected_target),
            (&py.None().into_bound(py), true),
        )
    })?;
    let connection_config: Bound<'_, PyDict> =
        effective_connection(&context, (&adapter_name, &base_connection))?.cast_into()?;
    let namespace: Bound<'_, PyTuple> = build_effective_target_namespace(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", context.discovered_inputs.clone()),
                ("selected_target", context.selected_target.clone()),
                (
                    "connection_database",
                    connection_config
                        .get_item("database")?
                        .unwrap_or_else(|| py.None().into_bound(py)),
                ),
                (
                    "connection_schema",
                    connection_config
                        .get_item("schema")?
                        .unwrap_or_else(|| py.None().into_bound(py)),
                ),
                (
                    "default_database",
                    context.adapter.call_method0("default_database")?,
                ),
                (
                    "default_schema",
                    context.adapter.call_method0("default_schema")?,
                ),
            ],
        )?),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?
    .cast_into()?;
    let target_config: Bound<'_, PyAny> = namespace.get_item(1)?;
    let connection_name: Option<String> = if target_config.is_none() {
        None
    } else {
        optional_string(&native_field(&target_config, "connection_name")?)?
    };
    let facts: DebugProjectFacts = DebugProjectFacts {
        project_file: path_text(py, &files.project_file)?,
        local_file: files
            .local_file
            .as_ref()
            .map(|path| path_text(py, path))
            .transpose()?,
        project_name: native_field(&context.discovered_inputs, "project_config")?
            .getattr("name")?
            .str()?
            .extract()?,
        adapter_name: adapter_name.str()?.extract()?,
        target_name: optional_string(&namespace.get_item(0)?)?.filter(|name| !name.is_empty()),
        connection_name,
        database: target_value(&namespace.get_item(2)?)?,
        schema: target_value(&namespace.get_item(3)?)?,
    };
    let warehouses: Vec<WarehouseCheck> =
        warehouse_checks(&context, (&command_scope, &base_connection))?;
    let check: ConnectionCheck = if check_connection {
        connection_check(&context.adapter, &connection_config)?
    } else {
        ConnectionCheck::Skipped
    };
    Ok(DebugReport {
        local_files: local_database_files(&facts.adapter_name, &connection_config)?,
        runtime: debug_runtime_lines(&runtime(py)?),
        configuration: debug_configuration_lines(&facts, &warehouses),
        providers: debug_provider_lines(&providers(&context.discovered_inputs)?),
        connection: debug_connection_lines(&connection_settings(&connection_config)?, &check),
    })
}

/// `sqb debug`: validate project config and the active connection; `1` when a check failed.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn run_debug_command(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<i32> {
    let arguments: KeywordArguments<'_> =
        KeywordArguments::read(py, "run_debug_command", options, DEBUG_OPTIONS)?;
    let project_dir: Bound<'_, PyAny> = arguments.value("project_dir")?;
    let project_dir: PathBuf = if project_dir.is_none() {
        imported(py, "pathlib", "Path")?
            .call_method0("cwd")?
            .extract()?
    } else {
        project_dir.extract()?
    };
    let json_output = arguments.flag("json_output")?;
    let build = || {
        build_report(
            py,
            &project_dir,
            (
                arguments.value("selected_target")?,
                !arguments.flag("no_connection")?,
            ),
        )
    };
    let report: DebugReport = if json_output {
        within(&redirect_stdout_to_stderr(py)?, |_| build())?
    } else {
        build()?
    };
    let text: String = if json_output {
        format_debug_json(&report) + "\n"
    } else {
        let use_color: bool = !arguments.flag("no_color")? && stdout_supports_color(py)?;
        format_debug_text(&report, CliStyle { use_color })
    };
    let stdout: Bound<'_, PyAny> = py.import("sys")?.getattr("stdout")?;
    stdout.call_method1("write", (text,))?;
    stdout.call_method0("flush")?;
    if let Some(failure) = debug_failure(&report) {
        report_failure_cause(FailureCause {
            error_type: failure.error_type,
            error_code: failure.error_code,
            message: failure.message,
        });
    }
    Ok(if report.success() { 0 } else { 1 })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_debug_command, module)?)
}
