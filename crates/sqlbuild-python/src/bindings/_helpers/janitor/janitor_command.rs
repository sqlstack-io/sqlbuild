//! `sqb janitor`: preview stale warehouse objects, confirm, and clean them up.

use pyo3::IntoPyObject;
use pyo3::exceptions::PyKeyboardInterrupt;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::janitor_planning::main::resolve_retention_days::resolve_retention_days;
use sqlbuild_cli::diff_report::errors::DiffUserError;
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::janitor_text::errors::JanitorSettingsError;
use sqlbuild_core::janitor_text::main::format_janitor_json::format_janitor_json;
use sqlbuild_core::janitor_text::main::janitor_confirmation_phrase::janitor_confirmation_phrase;
use sqlbuild_core::janitor_text::main::janitor_confirmation_prompt::janitor_confirmation_prompt;
use sqlbuild_core::janitor_text::main::janitor_disabled_text::janitor_disabled_text;
use sqlbuild_core::janitor_text::main::janitor_item_counts::janitor_item_counts;
use sqlbuild_core::janitor_text::main::janitor_nothing_to_do_text::janitor_nothing_to_do_text;
use sqlbuild_core::janitor_text::main::janitor_plan_has_work::janitor_plan_has_work;
use sqlbuild_core::janitor_text::main::janitor_preview_complete_text::janitor_preview_complete_text;
use sqlbuild_core::janitor_text::main::janitor_preview_start_text::janitor_preview_start_text;
use sqlbuild_core::janitor_text::main::render_janitor_completion::render_janitor_completion;
use sqlbuild_core::janitor_text::main::render_janitor_plan::render_janitor_plan;
use sqlbuild_core::janitor_text::main::validate_janitor_settings::validate_janitor_settings;
use sqlbuild_core::janitor_text::models::{JanitorOutcome, JanitorPlanText, JanitorRunText};
use std::time::Instant;

use crate::bindings::_helpers::cli::machine_output::{
    report_machine_output_written, take_machine_output_written,
};
use crate::bindings::_helpers::command_runtime::preview_target::validate_preview_target;
use crate::bindings::_helpers::compile_driver::discovery::discover_project_inputs;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_target_name;
use crate::bindings::_helpers::diff_report::support::cli_error;
use crate::bindings::_helpers::janitor::janitor_phases::{
    JanitorCompiled, build_janitor_execution_plan, close_janitor_warehouse,
    compile_janitor_project, connect_janitor_warehouse, execute_janitor_cleanup, output_stream,
};
use crate::bindings::_helpers::janitor::janitor_plan::{janitor_plan_text, janitor_result_counts};
use crate::bindings::_helpers::janitor::janitor_progress::JanitorProgressPrinter;
use crate::bindings::_helpers::planner_phases::objects::object;
use crate::bindings::_helpers::scenario_command::setup::stream_isatty;
use sqlbuild_python_base::presentation_text::main::stdout_supports_color::stdout_supports_color;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

const COMMAND_NAME: &str = "janitor";
/// The code of a cleanup that needs a confirmation stdin cannot give.
const CONFIRMATION_CODE: &str = "C503";
const CONFIRMATION_REFUSAL: &str =
    "janitor cleanup requires confirmation, but stdin is not a terminal";
const CONFIRMATION_HELP: &str = "Run sqb janitor in a terminal to confirm, or pass --auto-approve.";
const LINE_ENDINGS: [char; 2] = ['\n', '\r'];

fn imported<'py>(py: Python<'py>, module: &str, name: &str) -> PyResult<Bound<'py, PyAny>> {
    py.import(module)?.getattr(name)
}

fn keywords<'py>(
    py: Python<'py>,
    items: &[(&str, &Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyDict>> {
    let dict: Bound<'py, PyDict> = PyDict::new(py);
    for (name, value) in items {
        dict.set_item(*name, value)?;
    }
    Ok(dict)
}

fn settings_error(py: Python<'_>, error: JanitorSettingsError) -> PyErr {
    let raised = (|| {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("code", error.code)?;
        error_class(py, ErrorId::CliUser)?.call((error.message,), Some(&options))
    })();
    match raised {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

fn write(stream: &Bound<'_, PyAny>, text: &str) -> PyResult<()> {
    stream.call_method1("write", (text,))?;
    stream.call_method0("flush")?;
    Ok(())
}

/// Print the `--json` document on stdout when JSON output was requested.
fn write_json(
    (request, plan): (&Bound<'_, PyAny>, Option<&JanitorPlanText>),
    outcome: JanitorOutcome,
    run: Option<&JanitorRunText>,
    error: Option<&str>,
) -> PyResult<()> {
    if !native_field(request, "json_output")?.extract::<bool>()? {
        return Ok(());
    }
    let stdout: Bound<'_, PyAny> = imported(request.py(), "sys", "stdout")?;
    write(
        &stdout,
        &format!("{}\n", format_janitor_json(plan, outcome, run, error)),
    )?;
    report_machine_output_written();
    Ok(())
}

/// Discover the project and resolve the target and output context.
fn invocation<'py>(py: Python<'py>, request: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let project_dir: Bound<'py, PyAny> = match native_field(request, "project_dir")? {
        directory if !directory.is_none() => directory,
        _ => py.import("pathlib")?.getattr("Path")?.call_method0("cwd")?,
    };
    let discovered: Bound<'py, PyAny> =
        discover_project_inputs(py, Some(keywords(py, &[("project_dir", &project_dir)])?))?;
    let as_target: Bound<'py, PyAny> = native_field(request, "as_target")?;
    validate_preview_target(
        py,
        discovered.clone(),
        as_target.extract::<Option<String>>()?,
        COMMAND_NAME.to_owned(),
    )?;
    let selected_target: Bound<'py, PyAny> = native_field(request, "selected_target")?;
    let active_target: Bound<'py, PyAny> = object(
        py,
        resolve_target_name(
            py,
            &native_field(&discovered, "project_config")?,
            &native_field(&discovered, "local_config")?,
            &selected_target,
        )?,
    )?;
    let machine_output: bool = native_field(request, "json_output")?.extract()?;
    let use_color: bool = !machine_output
        && !native_field(request, "no_color")?.extract::<bool>()?
        && stdout_supports_color(py)?;
    let options: Bound<'py, PyDict> = keywords(
        py,
        &[
            ("effective_project_dir", &project_dir),
            ("discovered_inputs", &discovered),
            ("selected_target", &selected_target),
            ("active_target_name", &active_target),
            ("as_target", &as_target),
        ],
    )?;
    options.set_item("use_color", use_color)?;
    options.set_item("machine_output", machine_output)?;
    new_record(py, RecordId::JanitorInvocation, (), Some(&options))
}

/// Effective retention settings, validated.
fn settings<'py>(
    py: Python<'py>,
    request: &Bound<'py, PyAny>,
    janitor: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let retention_days: Bound<'py, PyAny> = resolve_retention_days(
        native_field(request, "retention_days")?.extract()?,
        native_field(janitor, "retention_days")?.extract()?,
    )
    .into_pyobject(py)?
    .into_any();
    validate_janitor_settings(retention_days.extract()?, 0)
        .map_err(|error| settings_error(py, error))?;
    let requested: Bound<'py, PyAny> = native_field(request, "direct_state_history_versions")?;
    let history: Bound<'py, PyAny> = if requested.is_none() {
        native_field(janitor, "direct_state_history_versions")?
    } else {
        requested
    };
    validate_janitor_settings(0, history.extract()?).map_err(|error| settings_error(py, error))?;
    let options: Bound<'py, PyDict> = keywords(
        py,
        &[
            ("retention_days", &retention_days),
            ("direct_state_history_versions", &history),
            (
                "archive_retention_days",
                &native_field(janitor, "archive_retention_days")?,
            ),
            (
                "drop_old_name_views",
                &native_field(request, "drop_old_name_views")?,
            ),
            ("in_use_window", &native_field(janitor, "in_use_window")?),
        ],
    )?;
    options.set_item(
        "keep_in_use",
        native_field(request, "keep_in_use")?.is_truthy()?
            || native_field(janitor, "keep_in_use")?.is_truthy()?,
    )?;
    new_record(py, RecordId::JanitorSettings, (), Some(&options))
}

/// Ask for the exact confirmation phrase; an interrupt cancels, and a non-interactive stdin refuses.
fn confirmed(stream: &Bound<'_, PyAny>, plan: &JanitorPlanText) -> PyResult<bool> {
    let py: Python<'_> = stream.py();
    let stdin: Bound<'_, PyAny> = imported(py, "sys", "stdin")?;
    if stdin.is_none() || !stream_isatty(&stdin)? {
        return Err(cli_error(
            py,
            ErrorId::CliUser,
            &DiffUserError {
                message: CONFIRMATION_REFUSAL.to_owned(),
                code: CONFIRMATION_CODE.to_owned(),
                help: Some(CONFIRMATION_HELP.to_owned()),
            },
        ));
    }
    write(stream, &janitor_confirmation_prompt(plan))?;
    match stdin.call_method0("readline") {
        Ok(response) => Ok(response.extract::<String>()?.trim_end_matches(LINE_ENDINGS)
            == janitor_confirmation_phrase(plan)),
        Err(error) if error.is_instance_of::<PyKeyboardInterrupt>(py) => {
            write(stream, "\n")?;
            Ok(false)
        }
        Err(error) => Err(error),
    }
}

/// Run the confirmed plan, streaming each action, then print the final line and `--json`.
fn apply(
    (request, invocation): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    (stream, style): (&Bound<'_, PyAny>, CliStyle),
    (compiled, connection): (&JanitorCompiled<'_>, &Bound<'_, PyAny>),
    (planning, plan): (&Bound<'_, PyAny>, &JanitorPlanText),
) -> PyResult<i32> {
    let py: Python<'_> = stream.py();
    let progress: Bound<'_, JanitorProgressPrinter> = Bound::new(
        py,
        JanitorProgressPrinter::new(stream, style, plan.clone())?,
    )?;
    let started: Instant = Instant::now();
    let executed: PyResult<Bound<'_, PyAny>> = execute_janitor_cleanup(
        invocation,
        (compiled, connection),
        (planning, progress.as_any()),
    );
    let elapsed_seconds: f64 = started.elapsed().as_secs_f64();
    let items = progress.get().items();
    match executed {
        Ok(result) => {
            let run: JanitorRunText = JanitorRunText {
                items,
                counts: janitor_result_counts(&result)?,
                elapsed_seconds,
            };
            write(
                stream,
                &format!(
                    "\n{}",
                    render_janitor_completion(&run.counts, Some(elapsed_seconds), false, style)
                ),
            )?;
            write_json(
                (request, Some(plan)),
                JanitorOutcome::Completed,
                Some(&run),
                None,
            )?;
            Ok(0)
        }
        Err(error) => {
            let run: JanitorRunText = JanitorRunText {
                counts: janitor_item_counts(&items),
                items,
                elapsed_seconds,
            };
            write(
                stream,
                &format!(
                    "\n{}",
                    render_janitor_completion(&run.counts, Some(elapsed_seconds), true, style)
                ),
            )?;
            let message: String = error.value(py).str()?.extract()?;
            write_json(
                (request, Some(plan)),
                JanitorOutcome::Failed,
                Some(&run),
                Some(&message),
            )?;
            Err(error)
        }
    }
}

/// Plan, preview, confirm and clean up with an open warehouse connection.
fn clean(
    request: &Bound<'_, PyAny>,
    (invocation, settings): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    (compiled, connection): (&JanitorCompiled<'_>, &Bound<'_, PyAny>),
) -> PyResult<i32> {
    let style: CliStyle = CliStyle {
        use_color: native_field(invocation, "use_color")?.extract()?,
    };
    let stream: Bound<'_, PyAny> = output_stream(invocation)?;
    let planning: Bound<'_, PyAny> =
        build_janitor_execution_plan((invocation, settings), compiled, connection)?;
    let plan: JanitorPlanText = janitor_plan_text(&planning)?;
    let verbose: bool = native_field(request, "verbose")?.extract()?;
    write(&stream, &render_janitor_plan(&plan, verbose, style))?;
    let blocked: i32 = i32::from(!plan.blocked_schemas.is_empty());
    let as_target: Option<String> = native_field(invocation, "as_target")?.extract()?;
    if let Some(as_target) = as_target {
        let active: Option<String> = native_field(invocation, "active_target_name")?.extract()?;
        write(
            &stream,
            &janitor_preview_complete_text(&as_target, active.as_deref(), style),
        )?;
        write_json(
            (request, Some(&plan)),
            JanitorOutcome::Previewed,
            None,
            None,
        )?;
        return Ok(blocked);
    }
    if blocked != 0 {
        write_json((request, Some(&plan)), JanitorOutcome::Blocked, None, None)?;
        return Ok(1);
    }
    if !janitor_plan_has_work(&plan) {
        write(&stream, &janitor_nothing_to_do_text(style))?;
        write_json(
            (request, Some(&plan)),
            JanitorOutcome::NothingToDo,
            None,
            None,
        )?;
        return Ok(0);
    }
    if !native_field(request, "auto_approve")?.extract::<bool>()? && !confirmed(&stream, &plan)? {
        write(&stream, "Janitor cancelled.\n")?;
        write_json(
            (request, Some(&plan)),
            JanitorOutcome::Cancelled,
            None,
            None,
        )?;
        return Ok(1);
    }
    apply(
        (request, invocation),
        (&stream, style),
        (compiled, connection),
        (&planning, &plan),
    )
}

/// Run one `sqb janitor` request.
#[pyfunction]
#[pyo3(signature = (*, request))]
pub(crate) fn run_janitor_command(py: Python<'_>, request: &Bound<'_, PyAny>) -> PyResult<i32> {
    let _ = take_machine_output_written();
    let invocation: Bound<'_, PyAny> = invocation(py, request)?;
    let style: CliStyle = CliStyle {
        use_color: native_field(&invocation, "use_color")?.extract()?,
    };
    let janitor: Bound<'_, PyAny> = native_field(&invocation, "discovered_inputs")?
        .getattr("project_config")?
        .getattr("janitor")?;
    let stream: Bound<'_, PyAny> = output_stream(&invocation)?;
    if !native_field(&janitor, "enabled")?.is_truthy()? {
        write(&stream, &janitor_disabled_text(style))?;
        write_json((request, None), JanitorOutcome::Disabled, None, None)?;
        return Ok(0);
    }
    let settings: Bound<'_, PyAny> = settings(py, request, &janitor)?;
    let as_target: Option<String> = native_field(&invocation, "as_target")?.extract()?;
    if let Some(as_target) = &as_target {
        let active: Option<String> = native_field(&invocation, "active_target_name")?.extract()?;
        write(
            &stream,
            &janitor_preview_start_text(as_target, active.as_deref()),
        )?;
    }
    let compiled: JanitorCompiled<'_> = compile_janitor_project(&invocation)?;
    let connection: Bound<'_, PyAny> = connect_janitor_warehouse(&invocation, &compiled)?;
    let outcome: PyResult<i32> = clean(request, (&invocation, &settings), (&compiled, &connection));
    let closed: PyResult<()> = close_janitor_warehouse(&compiled, &connection);
    let code: i32 = outcome?;
    closed?;
    Ok(code)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_janitor_command, module)?)?;
    Ok(())
}
