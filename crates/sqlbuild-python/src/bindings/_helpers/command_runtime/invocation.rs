//! Discovery, adapter, connection and output context for project commands that connect.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyBool;

use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::boundary::python_calls::{call_with, imported};
use crate::bindings::_helpers::build_progress::progress_reporters::ConnectionProgressReporter;
use crate::bindings::_helpers::build_progress::progress_reporters::PlanningProgressReporter;
use crate::bindings::_helpers::command_runtime::adapters::resolve_cli_adapter;
use crate::bindings::_helpers::command_runtime::connection::resolve_project_connection_config;
use crate::bindings::_helpers::compile_driver::discovery::discover_project_inputs;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_effective_adapter_name;
use crate::bindings::_helpers::presentation_text::terminal::stdout_supports_color;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;

/// The project directory, discovered inputs, adapter context, colour and progress stream.
struct InvocationContext<'py> {
    project_dir: Bound<'py, PyAny>,
    discovered: Bound<'py, PyAny>,
    adapter_context: Bound<'py, PyAny>,
    use_color: bool,
    stream: Bound<'py, PyAny>,
}

fn boolean(py: Python<'_>, value: bool) -> Bound<'_, PyAny> {
    PyBool::new(py, value).to_owned().into_any()
}

/// The effective adapter and project connection configuration as an `AdapterConnectionContext`.
pub(crate) fn resolve_adapter_connection_context<'py>(
    discovered: &Bound<'py, PyAny>,
    project_dir: &Bound<'py, PyAny>,
    (selected_target, cli_vars): (Bound<'py, PyAny>, Bound<'py, PyAny>),
    require_connection: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = discovered.py();
    let adapter_name: Bound<'py, PyAny> = resolve_effective_adapter_name(
        &native_field(discovered, "project_config")?,
        &native_field(discovered, "local_config")?,
    )?;
    let adapter: Bound<'py, PyAny> = resolve_cli_adapter(&adapter_name, project_dir)?;
    let connection_config: Bound<'py, PyAny> = resolve_project_connection_config(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered.clone()),
                ("project_dir", project_dir.clone()),
                ("selected_target", selected_target),
                ("cli_vars", cli_vars),
                ("require_connection", boolean(py, require_connection)),
            ],
        )?),
    )?;
    new_record_with(
        py,
        RecordId::AdapterConnectionContext,
        &[
            ("adapter_name", adapter_name),
            ("adapter", adapter),
            ("connection_config", connection_config),
        ],
    )
}

fn invocation_context<'py>(
    request: &Bound<'py, PyAny>,
    (require_connection, verified): (bool, Option<&Bound<'py, PyAny>>),
) -> PyResult<InvocationContext<'py>> {
    let py: Python<'py> = request.py();
    let requested: Bound<'py, PyAny> = native_field(request, "project_dir")?;
    let project_dir: Bound<'py, PyAny> = if requested.is_none() {
        imported(py, "pathlib", "Path")?.call_method0("cwd")?
    } else {
        requested
    };
    let discovered: Bound<'py, PyAny> = match verified {
        Some(discovered) => discovered.clone(),
        None => discover_project_inputs(
            py,
            Some(keywords(py, &[("project_dir", project_dir.clone())])?),
        )?,
    };
    let adapter_context: Bound<'py, PyAny> = resolve_adapter_connection_context(
        &discovered,
        &project_dir,
        (
            native_field(request, "selected_target")?,
            native_field(request, "cli_vars")?,
        ),
        require_connection,
    )?;
    let machine_output: bool = native_field(request, "json_output")?.is_truthy()?;
    let use_color: bool = !native_field(request, "no_color")?.is_truthy()?
        && !machine_output
        && stdout_supports_color(py)?;
    let stream: Bound<'py, PyAny> =
        py.import("sys")?
            .getattr(if machine_output { "stderr" } else { "stdout" })?;
    Ok(InvocationContext {
        project_dir,
        discovered,
        adapter_context,
        use_color,
        stream,
    })
}

fn context_fields<'py>(
    context: &InvocationContext<'py>,
) -> PyResult<Vec<(&'static str, Bound<'py, PyAny>)>> {
    let InvocationContext {
        project_dir,
        discovered,
        adapter_context,
        use_color,
        stream,
    } = context;
    let py: Python<'py> = project_dir.py();
    Ok(vec![
        ("effective_project_dir", project_dir.clone()),
        ("discovered_inputs", discovered.clone()),
        (
            "adapter_name",
            native_field(adapter_context, "adapter_name")?,
        ),
        ("adapter", native_field(adapter_context, "adapter")?),
        (
            "connection_config",
            native_field(adapter_context, "connection_config")?,
        ),
        ("use_color", boolean(py, *use_color)),
        ("progress_stream", stream.clone()),
    ])
}

/// `invocation_type(...)` with discovery (or a worker's `verified` inputs), adapter, connection and output context.
pub(crate) fn resolve_command_invocation<'py>(
    request: &Bound<'py, PyAny>,
    invocation_type: &Bound<'py, PyAny>,
    verified: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let context: InvocationContext<'py> = invocation_context(request, (true, verified))?;
    call_with(invocation_type, &context_fields(&context)?)
}

/// `invocation_type(...)` with command context plus connection and planning progress reporters.
pub(crate) fn resolve_reported_command_invocation<'py>(
    request: &Bound<'py, PyAny>,
    invocation_type: &Bound<'py, PyAny>,
    (require_connection, verified): (bool, Option<&Bound<'py, PyAny>>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let context: InvocationContext<'py> =
        invocation_context(request, (require_connection, verified))?;
    let InvocationContext {
        adapter_context,
        use_color,
        stream,
        ..
    } = &context;
    let mut fields: Vec<(&str, Bound<'py, PyAny>)> = context_fields(&context)?;
    fields.push((
        "connection_progress",
        ConnectionProgressReporter::new(
            py,
            Some(keywords(
                py,
                &[
                    (
                        "adapter_name",
                        native_field(adapter_context, "adapter_name")?,
                    ),
                    ("stream", stream.clone()),
                    ("use_color", boolean(py, *use_color)),
                ],
            )?),
        )
        .and_then(|value| pyo3::Bound::new(py, value))
        .map(pyo3::Bound::into_any)?,
    ));
    fields.push((
        "planning_progress",
        PlanningProgressReporter::new(
            py,
            &stream.clone(),
            boolean(py, *use_color).extract::<bool>()?,
        )
        .and_then(|value| pyo3::Bound::new(py, value))
        .map(pyo3::Bound::into_any)?,
    ));
    call_with(invocation_type, &fields)
}
