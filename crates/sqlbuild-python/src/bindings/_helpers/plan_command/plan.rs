//! `sqb plan`: compile the plan pipeline and print the plan as text or JSON.

use std::sync::Arc;

use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python, pyfunction,
};
use pyo3::types::{PyBool, PyCFunction, PyDict, PyDictMethods};
use pyo3::wrap_pyfunction;
use sqlbuild_config::command_connection::errors::CommandConnectionError;
use sqlbuild_core::plan_display::constants::{
    SYNCED_COPY_DEFER_HELP, SYNCED_COPY_DEFER_REFUSAL, SYNCED_COPY_KEY, SYNCED_COPY_READING,
};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_state::warehouse_state::models::WarehouseState;

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
use crate::bindings::_helpers::compile_driver::plan_pipeline::planned_project;
use crate::bindings::_helpers::compile_driver::rules_at_load::RulesAtLoadScope;
use crate::bindings::_helpers::consumption::policy::{changed_policy, policy_line};
use crate::bindings::_helpers::consumption::recorder::{ChangedDecision, preview_changed};
use crate::bindings::_helpers::dbt_interop::reference_resolver::resolve_external_sql_reference_resolver;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::native_host::live_reads::LiveReads;
use crate::bindings::_helpers::plan_command::changed::{changed_json, changed_text};
use crate::bindings::_helpers::plan_output::plan_json::{CHANGED_KEY, format_plan_json};
use crate::bindings::_helpers::plan_output::plan_text::format_plan;
use crate::bindings::_helpers::planner_warehouse::reads::PlannerReads;
use crate::bindings::_helpers::planner_warehouse::synced_reads::SyncedReads;
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

/// Write `text` to the progress stream and flush it, so it shows before slow work starts.
fn write_progress(stream: &Bound<'_, PyAny>, text: &str) -> PyResult<()> {
    stream.call_method1("write", (text,))?;
    stream.call_method0("flush")?;
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
    /// Set for a location's plan preview: the reads that answer from the synced state.
    preview: Option<SyncedPlan<'py>>,
}

/// A plan preview's reads over the synced state, and the handle the planner reads through.
struct SyncedPlan<'py> {
    synced: Arc<SyncedReads>,
    reads: Bound<'py, PlannerReads>,
}

/// A plan's project: compiled for the request, or the discovered inputs and compiled phase a location holds.
#[derive(Clone, Copy)]
pub(crate) enum PlanProject<'a, 'py> {
    Compiled,
    Held {
        discovered: &'a Bound<'py, PyAny>,
        compiled: &'a Bound<'py, PyAny>,
    },
}

/// A location preview's reads over the synced `state`, never deferring; other plans read live.
fn preview<'py>(
    request: &Bound<'py, PyAny>,
    state: Option<&Arc<WarehouseState>>,
) -> PyResult<Option<SyncedPlan<'py>>> {
    let py: Python<'py> = request.py();
    let Some(state) = state else {
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
    let synced: Arc<SyncedReads> = Arc::new(SyncedReads::new(Arc::clone(state)));
    let reads: Bound<'py, PlannerReads> = Bound::new(py, PlannerReads::new(synced.clone()))?;
    Ok(Some(SyncedPlan { synced, reads }))
}

fn resolve_invocation<'py>(
    request: &Bound<'py, PyAny>,
    (project, state): (PlanProject<'_, 'py>, Option<&Arc<WarehouseState>>),
) -> PyResult<PlanInvocation<'py>> {
    let py: Python<'py> = request.py();
    let requested_dir: Bound<'py, PyAny> = native_field(request, "project_dir")?;
    let project_dir: Bound<'py, PyAny> = if requested_dir.is_none() {
        imported(py, "pathlib", "Path")?.call_method0("cwd")?
    } else {
        requested_dir
    };
    let discovered: Bound<'py, PyAny> = match project {
        PlanProject::Held { discovered, .. } => discovered.clone(),
        PlanProject::Compiled => discover_project_inputs(
            py,
            Some(keywords(
                py,
                &[
                    ("project_dir", project_dir.clone()),
                    ("extract_output_column_locations", flag(py, false)),
                ],
            )?),
        )?,
    };
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
    let preview: Option<SyncedPlan<'py>> = preview(request, state)?;
    Ok(PlanInvocation {
        project_dir,
        discovered,
        context,
        stream,
        use_color,
        connection_progress,
        planning_progress,
        load_sources,
        preview,
    })
}

/// A `ConnectionHooks` forwarding to the plan's progress reporters; a preview opens no connection.
fn connection_hooks<'py>(invocation: &PlanInvocation<'py>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = invocation.stream.py();
    let on_progress: Bound<'py, PyAny> =
        native_field(&invocation.planning_progress, "on_progress")?;
    if invocation.preview.is_some() {
        let progress: Py<PlanningProgressReporter> = invocation
            .planning_progress
            .downcast::<PlanningProgressReporter>()?
            .clone()
            .unbind();
        let reading = PyCFunction::new_closure(py, None, None, move |arguments, _keywords| {
            progress
                .get()
                .on_progress(arguments.py(), SYNCED_COPY_READING)
        })?;
        return call_with(
            &host::entry(py, ExecutorHost::ConnectionHooks)?,
            &[
                ("on_progress", on_progress),
                ("on_connection_start", reading.into_any()),
            ],
        );
    }
    let connection: &Bound<'py, PyAny> = &invocation.connection_progress;
    call_with(
        &host::entry(py, ExecutorHost::ConnectionHooks)?,
        &[
            ("on_progress", on_progress),
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
    project: PlanProject<'_, 'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let resolver: Bound<'py, PyAny> = resolve_external_sql_reference_resolver(
        invocation.project_dir.clone(),
        invocation.discovered.clone(),
    )?;
    let as_target: Bound<'py, PyAny> = native_field(request, "as_target")?;
    if !as_target.is_none() {
        write_progress(
            &invocation.stream,
            &format!(
                "Previewing plan as target '{}' through the active target's connection (inspection only).\n",
                as_target.str()?
            ),
        )?;
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
    let adapter: Bound<'py, PyAny> = native_field(&invocation.context, "adapter")?;
    let preview: Option<Bound<'py, PyAny>> = invocation
        .preview
        .as_ref()
        .map(|preview| preview.reads.clone().into_any());
    planned_project(
        (&invocation.discovered, &adapter),
        (&pipeline_options, connection_hooks(invocation)?),
        (
            match project {
                PlanProject::Held { compiled, .. } => Some(compiled),
                PlanProject::Compiled => None,
            },
            preview.as_ref(),
        ),
    )
}

/// What `--changed` would run now, or None without `--changed`; read warnings go to the stream.
fn changed_preview(
    request: &Bound<'_, PyAny>,
    invocation: &PlanInvocation<'_>,
    pipeline: &Bound<'_, PyAny>,
) -> PyResult<Option<ChangedDecision>> {
    let project: Bound<'_, PyAny> = native_field(pipeline, "project")?;
    let Some(policy) = changed_policy(&invocation.project_dir, request, &project)? else {
        return Ok(None);
    };
    let plan: (&Bound<'_, PyAny>, &Bound<'_, PyAny>) =
        (&native_field(pipeline, "plan_output")?, &project);
    let full_refresh: bool = native_field(request, "full_refresh")?.is_truthy()?;
    let py = request.py();
    let reads: Bound<'_, PlannerReads> = match &invocation.preview {
        Some(preview) => preview.reads.clone(),
        None => LiveReads::handle(
            &native_field(&invocation.context, "adapter")?,
            &py.None().into_bound(py),
            Some(&native_field(&invocation.context, "connection_config")?),
        )?,
    };
    let changed = preview_changed(plan, reads.get().reads(), (&policy, full_refresh));
    reads.get().reads().close_consumption(py)?;
    let (decision, warnings) = changed?;
    for warning in warnings {
        write_progress(&invocation.stream, &format!("warning: {warning}\n"))?;
    }
    Ok(Some(decision))
}

/// Print the plan; the JSON plan is returned instead when `captured`.
fn write_output(
    request: &Bound<'_, PyAny>,
    (invocation, captured): (&PlanInvocation<'_>, bool),
    pipeline: &Bound<'_, PyAny>,
    changed: Option<&ChangedDecision>,
) -> PyResult<Option<String>> {
    let py: Python<'_> = request.py();
    let plan: Bound<'_, PyAny> = native_field(pipeline, "plan_output")?;
    let python_entries: Bound<'_, PyAny> = native_field(pipeline, "python_plan_entries")?;
    if native_field(request, "json_output")?.is_truthy()? {
        let mut options: Vec<(&str, Bound<'_, PyAny>)> =
            vec![("plan", plan), ("python_plan_entries", python_entries)];
        if let Some(facade) = &invocation.preview {
            options.push((
                SYNCED_COPY_KEY,
                synced_copy_json(py, &facade.synced)?.into_any(),
            ));
        }
        if let Some(decision) = changed {
            options.push((CHANGED_KEY, changed_json(py, decision)?.into_any()));
        }
        let text: String = format_plan_json(py, Some(keywords(py, &options)?))?;
        if captured {
            return Ok(Some(text));
        }
        imported(py, "builtins", "print")?.call1((text,))?;
        return Ok(None);
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
    let mut options: Vec<(&str, Bound<'_, PyAny>)> = vec![
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
    ];
    if let Some(decision) = changed {
        print_line(py, &policy_line(&decision.policy))?;
        options.push((
            "changed_sections",
            pyo3::types::PyString::new(py, &changed_text(decision, invocation.use_color))
                .into_any(),
        ));
    }
    let text: String = format_plan(py, Some(keywords(py, &options)?))?;
    print_line(py, &format!("\n{text}"))?;
    if let Some(facade) = &invocation.preview {
        print_line(py, &synced_copy_text(&facade.synced))?;
    }
    Ok(None)
}

/// The plan JSON's account of the synced copy: when it synced, window starts and unknowns.
fn synced_copy_json<'py>(py: Python<'py>, facade: &SyncedReads) -> PyResult<Bound<'py, PyDict>> {
    let synced: Bound<'py, PyDict> = PyDict::new(py);
    synced.set_item("synced_at", facade.synced_at())?;
    synced.set_item("cursor_window_starts", facade.windows())?;
    synced.set_item("unknown_until_run", facade.unknowns())?;
    Ok(synced)
}

/// The preview footer: when the copy was synced and what stays unknown until the run.
fn synced_copy_text(facade: &SyncedReads) -> String {
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
    Ok(plan(request, (PlanProject::Compiled, None))?.0)
}

/// A location's preview from the synced `state`, without rules, which ran at load; vars or a dbt manifest recompile.
pub(crate) fn location_plan<'py>(
    request: &Bound<'py, PyAny>,
    (held, state): (PlanProject<'_, 'py>, &Arc<WarehouseState>),
) -> PyResult<(i32, Option<String>)> {
    let external_manifest: bool = match held {
        PlanProject::Held { discovered, .. } => !native_field(
            &native_field(&native_field(discovered, "project_config")?, "dbt")?,
            "target_path",
        )?
        .is_none(),
        PlanProject::Compiled => false,
    };
    let _rules = RulesAtLoadScope::enter();
    let project: PlanProject<'_, 'py> =
        if external_manifest || native_field(request, "cli_vars")?.is_truthy()? {
            PlanProject::Compiled
        } else {
            held
        };
    plan(request, (project, Some(state)))
}

/// One plan: printed, or from a synced state, the JSON plan returned.
fn plan<'py>(
    request: &Bound<'py, PyAny>,
    (project, state): (PlanProject<'_, 'py>, Option<&Arc<WarehouseState>>),
) -> PyResult<(i32, Option<String>)> {
    let py: Python<'py> = request.py();
    let invocation: PlanInvocation<'py> = resolve_invocation(request, (project, state))?;
    if !native_field(request, "json_output")?.is_truthy()? {
        write_progress(&invocation.stream, "\n")?;
    }
    let verbose: bool = native_field(request, "verbose")?.is_truthy()?;
    let pipeline: Bound<'py, PyAny> = with_verbose_inspection(py, verbose, || {
        compile_pipeline(request, &invocation, project)
    })?;
    let changed: Option<ChangedDecision> = changed_preview(request, &invocation, &pipeline)?;
    let output: Option<String> = write_output(
        request,
        (&invocation, state.is_some()),
        &pipeline,
        changed.as_ref(),
    )?;
    Ok((
        i32::from(plan_has_errors(&native_field(&pipeline, "plan_output")?)?),
        output,
    ))
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
