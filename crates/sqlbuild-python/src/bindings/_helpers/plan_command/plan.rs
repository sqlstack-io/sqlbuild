//! `sqb plan`: compile the plan pipeline and print the plan as text or JSON.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python, pyfunction,
};
use pyo3::types::{PyBool, PyDict, PyDictMethods};
use pyo3::wrap_pyfunction;
use sqlbuild_config::command_connection::errors::CommandConnectionError;
use sqlbuild_core::plan_display::constants::{
    PLAN_SYNCED_COPY_ENV, SYNCED_COPY_DEFER_HELP, SYNCED_COPY_DEFER_REFUSAL, SYNCED_COPY_KEY,
};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::boundary::python_calls::{call_with, imported};
use crate::bindings::_helpers::build_command::support::keyword_relay;
use crate::bindings::_helpers::build_progress::progress_reporters::{
    ConnectionProgressReporter, PlanningProgressReporter,
};
use crate::bindings::_helpers::build_progress::progress_text::with_verbose_inspection;
use crate::bindings::_helpers::command_runtime::invocation::resolve_adapter_connection_context;
use crate::bindings::_helpers::command_runtime::preview_target::validate_preview_target;
use crate::bindings::_helpers::command_runtime::runtime_errors::raised;
use crate::bindings::_helpers::compile_attachment::effective_config::build_effective_settings_config;
use crate::bindings::_helpers::compile_driver::discovery::discover_project_inputs;
use crate::bindings::_helpers::compile_driver::plan_pipeline::run_compile_pipeline;
use crate::bindings::_helpers::dbt_interop::reference_resolver::resolve_external_sql_reference_resolver;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::native_host::synced_copy::SyncedCopyAdapter;
use crate::bindings::_helpers::plan_output::plan_json::format_plan_json;
use crate::bindings::_helpers::plan_output::plan_text::format_plan;
use crate::bindings::_helpers::presentation_text::terminal::stdout_supports_color;
use crate::bindings::_helpers::records::construct::new_record;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;

const COMMAND_NAME: &str = "plan";
const DEFAULT_SECTION_ENTRIES: i64 = 50;
/// The `severity` value of a plan finding that blocks a build.
const ERROR_SEVERITY: &str = "error";
const CONNECTION_TIMING: [&str; 2] = ["connection_count", "elapsed_seconds"];
/// Request fields passed to the compile pipeline under the same name.
const PIPELINE_FIELDS: [&str; 11] = [
    "no_sql_validation",
    "no_cache",
    "defer_to",
    "defer_sources_to",
    "cursor_overrides",
    "full_refresh",
    "select",
    "exclude",
    "cli_vars",
    "max_microbatches",
    "selection_diagnostics",
];

fn flag(py: Python<'_>, value: bool) -> Bound<'_, PyAny> {
    PyBool::new(py, value).to_owned().into_any()
}

fn print_line(py: Python<'_>, text: &str) -> PyResult<()> {
    imported(py, "builtins", "print")?.call1((text,))?;
    Ok(())
}

/// The discovered project, adapter context, output stream and progress reporters of one plan.
struct PlanInvocation<'py> {
    project_dir: Bound<'py, PyAny>,
    discovered: Bound<'py, PyAny>,
    context: Bound<'py, PyAny>,
    stream: Bound<'py, PyAny>,
    use_color: bool,
    connection_progress: Bound<'py, PyAny>,
    planning_progress: Bound<'py, PyAny>,
    load_sources: Bound<'py, PyAny>,
    /// Set for a location's plan preview: the adapter that answers from the synced copy.
    synced_copy: Option<Bound<'py, SyncedCopyAdapter>>,
}

/// The synced-copy adapter when a location asked for a preview; a preview never defers.
fn synced_copy<'py>(
    request: &Bound<'py, PyAny>,
    adapter: Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, SyncedCopyAdapter>>> {
    let py: Python<'py> = request.py();
    let Ok(path) = std::env::var(PLAN_SYNCED_COPY_ENV) else {
        return Ok(None);
    };
    if native_field(request, "defer_to")?.is_truthy()? {
        return Err(raised(
            py,
            CommandConnectionError::User {
                code: None,
                message: SYNCED_COPY_DEFER_REFUSAL.to_owned(),
                help: Some(SYNCED_COPY_DEFER_HELP.to_owned()),
            },
        ));
    }
    Ok(Some(Bound::new(
        py,
        SyncedCopyAdapter::load(py, adapter, &path)?,
    )?))
}

fn resolve_invocation<'py>(request: &Bound<'py, PyAny>) -> PyResult<PlanInvocation<'py>> {
    let py: Python<'py> = request.py();
    let requested_dir: Bound<'py, PyAny> = native_field(request, "project_dir")?;
    let project_dir: Bound<'py, PyAny> = if requested_dir.is_none() {
        imported(py, "pathlib", "Path")?.call_method0("cwd")?
    } else {
        requested_dir
    };
    let discovered: Bound<'py, PyAny> = discover_project_inputs(
        py,
        Some(keywords(
            py,
            &[
                ("project_dir", project_dir.clone()),
                ("extract_output_column_locations", flag(py, false)),
            ],
        )?),
    )?;
    validate_preview_target(
        py,
        discovered.clone(),
        native_field(request, "as_target")?.extract()?,
        COMMAND_NAME.to_owned(),
    )?;
    let context: Bound<'py, PyAny> = resolve_adapter_connection_context(
        &discovered,
        &project_dir,
        (
            native_field(request, "selected_target")?,
            native_field(request, "cli_vars")?,
        ),
        true,
    )?;
    let json_output: bool = native_field(request, "json_output")?.is_truthy()?;
    let use_color: bool = !native_field(request, "no_color")?.is_truthy()?
        && !json_output
        && stdout_supports_color(py)?;
    let stream: Bound<'py, PyAny> =
        py.import("sys")?
            .getattr(if json_output { "stderr" } else { "stdout" })?;
    let connection_progress: Bound<'py, PyAny> = Bound::new(
        py,
        ConnectionProgressReporter::new(
            py,
            Some(keywords(
                py,
                &[
                    ("adapter_name", native_field(&context, "adapter_name")?),
                    ("stream", stream.clone()),
                    ("use_color", flag(py, use_color)),
                ],
            )?),
        )?,
    )?
    .into_any();
    let planning_progress: Bound<'py, PyAny> =
        Bound::new(py, PlanningProgressReporter::new(py, &stream, use_color)?)?.into_any();
    let requested_load: Bound<'py, PyAny> = native_field(request, "load_sources")?;
    let load_sources: Bound<'py, PyAny> = if requested_load.is_none() {
        native_field(
            &build_effective_settings_config(&discovered)?,
            "auto_load_sources",
        )?
    } else {
        requested_load
    };
    let synced_copy: Option<Bound<'py, SyncedCopyAdapter>> =
        synced_copy(request, native_field(&context, "adapter")?)?;
    Ok(PlanInvocation {
        project_dir,
        discovered,
        context,
        stream,
        use_color,
        connection_progress,
        planning_progress,
        load_sources,
        synced_copy,
    })
}

/// A `ConnectionHooks` forwarding to the plan's progress reporters.
fn connection_hooks<'py>(invocation: &PlanInvocation<'py>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = invocation.stream.py();
    let connection: &Bound<'py, PyAny> = &invocation.connection_progress;
    call_with(
        &host::entry(py, ExecutorHost::ConnectionHooks)?,
        &[
            (
                "on_progress",
                native_field(&invocation.planning_progress, "on_progress")?,
            ),
            (
                "on_connection_start",
                native_field(connection, "on_connection_start")?,
            ),
            (
                "on_connection_complete",
                keyword_relay(
                    native_field(connection, "on_connection_complete")?,
                    &CONNECTION_TIMING,
                )?,
            ),
            (
                "on_connection_error",
                keyword_relay(
                    native_field(connection, "on_connection_error")?,
                    &CONNECTION_TIMING,
                )?,
            ),
        ],
    )
}

fn compile_pipeline<'py>(
    request: &Bound<'py, PyAny>,
    invocation: &PlanInvocation<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let resolver: Bound<'py, PyAny> = resolve_external_sql_reference_resolver(
        invocation.project_dir.clone(),
        invocation.discovered.clone(),
    )?;
    let as_target: Bound<'py, PyAny> = native_field(request, "as_target")?;
    if !as_target.is_none() {
        invocation.stream.call_method1(
            "write",
            (format!(
                "Previewing plan as target '{}' through the active target's connection (inspection only).\n",
                as_target.str()?
            ),),
        )?;
        invocation.stream.call_method0("flush")?;
    }
    let options: Bound<'py, PyDict> = PyDict::new(py);
    options.set_item(
        "selected_target",
        if as_target.is_none() {
            native_field(request, "selected_target")?
        } else {
            as_target
        },
    )?;
    for field in PIPELINE_FIELDS {
        options.set_item(field, native_field(request, field)?)?;
    }
    options.set_item("auto_load_sources", &invocation.load_sources)?;
    options.set_item(
        "connection_config",
        native_field(&invocation.context, "connection_config")?,
    )?;
    options.set_item("external_sql_reference_resolver", resolver)?;
    options.set_item(
        "resolve_python_run_selectors",
        native_field(request, "include_python")?,
    )?;
    options.set_item("record_migration_fingerprints", false)?;
    let pipeline_options: Bound<'py, PyAny> =
        new_record(py, RecordId::CompilePipelineOptions, (), Some(&options))?;
    run_compile_pipeline(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", invocation.discovered.clone()),
                (
                    "adapter",
                    match &invocation.synced_copy {
                        Some(facade) => facade.clone().into_any(),
                        None => native_field(&invocation.context, "adapter")?,
                    },
                ),
                ("options", pipeline_options),
                ("hooks", connection_hooks(invocation)?),
            ],
        )?),
    )
}

fn write_output(
    request: &Bound<'_, PyAny>,
    invocation: &PlanInvocation<'_>,
    pipeline: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py: Python<'_> = request.py();
    let plan: Bound<'_, PyAny> = native_field(pipeline, "plan_output")?;
    let python_entries: Bound<'_, PyAny> = native_field(pipeline, "python_plan_entries")?;
    if native_field(request, "json_output")?.is_truthy()? {
        let mut options: Vec<(&str, Bound<'_, PyAny>)> =
            vec![("plan", plan), ("python_plan_entries", python_entries)];
        if let Some(facade) = &invocation.synced_copy {
            options.push((
                SYNCED_COPY_KEY,
                synced_copy_json(py, facade.get())?.into_any(),
            ));
        }
        let text: String = format_plan_json(py, Some(keywords(py, &options)?))?;
        imported(py, "builtins", "print")?.call1((text,))?;
        return Ok(());
    }
    let display: Bound<'_, PyDict> = PyDict::new(py);
    display.set_item(
        "max_entries_per_section",
        if native_field(request, "verbose")?.is_truthy()? {
            None
        } else {
            Some(DEFAULT_SECTION_ENTRIES)
        },
    )?;
    let text: String = format_plan(
        py,
        Some(keywords(
            py,
            &[
                ("plan", plan),
                ("full_refresh", native_field(request, "full_refresh")?),
                ("use_color", flag(py, invocation.use_color)),
                (
                    "display_options",
                    new_record(py, RecordId::DisplayOptions, (), Some(&display))?,
                ),
                ("python_plan_entries", python_entries),
                ("include_direct_freshness_diagnostics", flag(py, false)),
                ("include_completion", flag(py, true)),
            ],
        )?),
    )?;
    print_line(py, &format!("\n{text}"))?;
    if let Some(facade) = &invocation.synced_copy {
        print_line(py, &synced_copy_text(facade.get()))?;
    }
    Ok(())
}

/// The plan JSON's account of the synced copy: when it synced, window starts and unknowns.
fn synced_copy_json<'py>(
    py: Python<'py>,
    facade: &SyncedCopyAdapter,
) -> PyResult<Bound<'py, PyDict>> {
    let synced: Bound<'py, PyDict> = PyDict::new(py);
    synced.set_item("synced_at", facade.synced_at())?;
    synced.set_item("cursor_window_starts", facade.windows())?;
    synced.set_item("unknown_until_run", facade.unknowns())?;
    Ok(synced)
}

/// The preview footer: when the copy was synced and what stays unknown until the run.
fn synced_copy_text(facade: &SyncedCopyAdapter) -> String {
    let mut lines: Vec<String> = vec![format!(
        "Planned from the synced copy ({}); the run re-plans live.",
        facade.synced_at().map_or_else(
            || "not fully synced yet".to_owned(),
            |at| format!("synced {at}")
        )
    )];
    let windows = facade.windows();
    if !windows.is_empty() {
        lines.push("Cursor windows (up to the latest data when the run starts):".to_owned());
        lines.extend(
            windows
                .iter()
                .map(|(model, start)| format!("  - {model}: from {start}")),
        );
    }
    lines.push("Unknown until run:".to_owned());
    lines.extend(
        facade
            .unknowns()
            .into_iter()
            .map(|item| format!("  - {item}")),
    );
    lines.join("\n")
}

/// Run one `sqb plan` request.
#[pyfunction]
#[pyo3(signature = (*, request))]
pub(crate) fn run_plan_command(request: &Bound<'_, PyAny>) -> PyResult<i32> {
    let py: Python<'_> = request.py();
    let invocation: PlanInvocation<'_> = resolve_invocation(request)?;
    if !native_field(request, "json_output")?.is_truthy()? {
        invocation.stream.call_method1("write", ("\n",))?;
        invocation.stream.call_method0("flush")?;
    }
    let verbose: bool = native_field(request, "verbose")?.is_truthy()?;
    let pipeline: Bound<'_, PyAny> =
        with_verbose_inspection(py, verbose, || compile_pipeline(request, &invocation))?;
    write_output(request, &invocation, &pipeline)?;
    Ok(i32::from(plan_has_errors(&native_field(
        &pipeline,
        "plan_output",
    )?)?))
}

/// Whether any plan finding has error severity, which `sqb build` refuses.
fn plan_has_errors(plan: &Bound<'_, PyAny>) -> PyResult<bool> {
    for warning in native_field(plan, "warnings")?.try_iter()? {
        let severity: Bound<'_, PyAny> =
            native_field(&native_field(&warning?, "severity")?, "value")?;
        if severity.extract::<String>()? == ERROR_SEVERITY {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_plan_command, module)?)
}
