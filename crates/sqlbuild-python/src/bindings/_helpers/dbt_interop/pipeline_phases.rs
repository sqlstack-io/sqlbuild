//! dbt execution phases, live SQLBuild planning and executor handoff.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{IntoPyObject, pyfunction, wrap_pyfunction};

use crate::bindings::_helpers::dbt_interop::execution::render_dbt_execution_summary_footer;
use crate::bindings::_helpers::dbt_interop::pipeline_prologue::progress;
use crate::bindings::_helpers::dbt_interop::plan::render_dbt_interop_plan;
use crate::bindings::_helpers::dbt_interop::plan_output::build_sqlbuild_plan_output;
use crate::bindings::_helpers::dbt_interop::plan_output::{connection_config, connection_hooks};
use crate::bindings::_helpers::dbt_interop::runtime_execution::execute_dbt_commands;
use crate::bindings::_helpers::dbt_interop::sqlbuild_work::execute_dbt_sqlbuild_work;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::build_command::main::boolean::boolean;
use sqlbuild_python_base::build_command::main::emit::emit;
use sqlbuild_python_base::build_command::main::none::none;
use sqlbuild_python_base::build_command::main::text::text;
use sqlbuild_python_base::build_command::main::tuple::tuple;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced::replaced;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::command_runtime::main::resolve_concurrency::resolve_concurrency;
use sqlbuild_python_front::contracts::main::resolve_target_config::resolve_target_config;
use sqlbuild_python_output::build_progress::models::ConnectionProgressReporter;

const OPTIONS: (&[&str], &[&str]) = (
    &["request", "invocation"],
    &[
        "compiled",
        "plan",
        "merged_dbt_argv",
        "execution",
        "manifest",
        "failed_sqlbuild_model_names",
        "plan_output",
        "connection_config",
        "message",
    ],
);

fn environment<'py>(args: &KeywordArguments<'py>) -> PyResult<Bound<'py, PyAny>> {
    let request = args.value("request")?;
    let compiled = args.value("compiled")?;
    new_record_with(
        request.py(),
        RecordId::DbtPlanEnvironment,
        &[
            ("project_dir", native_field(&request, "project_dir")?),
            (
                "discovered_inputs",
                args.value("invocation")?.getattr("discovered_inputs")?,
            ),
            ("project", native_field(&compiled, "project")?),
            ("adapter", native_field(&compiled, "adapter")?),
            ("adapter_name", native_field(&compiled, "adapter_name")?),
        ],
    )
}

/// Resolve the selected SQLBuild work's effective connection configuration.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn resolve_dbt_connection_config<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(py, "resolve_dbt_connection_config", options, OPTIONS)?;
    connection_config(&environment(&args)?)
}

/// Render the merged selection unless the command is producing JSON.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn write_dbt_execution_plan_text(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<()> {
    let args = KeywordArguments::read(py, "write_dbt_execution_plan_text", options, OPTIONS)?;
    let request = args.value("request")?;
    if native_field(&request, "json_output")?.is_truthy()? {
        return Ok(());
    }
    let plan = args.value("plan")?;
    let merged = args.value("merged_dbt_argv")?;
    let display = if merged.is_none() {
        plan
    } else {
        replaced(
            &plan,
            &[
                ("dbt_command_argv", merged),
                ("supplemental_dbt_command_argvs", tuple(py, &[])?),
            ],
        )?
    };
    let limit = if native_field(&request, "verbose")?.is_truthy()? {
        none(py)
    } else {
        pyo3::types::PyInt::new(py, super::constants::PLAN_DISPLAY_LIMIT).into_any()
    };
    let rendered = render_dbt_interop_plan(
        py,
        Some(keywords(
            py,
            &[
                ("plan", display),
                ("json_output", boolean(py, false)),
                ("use_color", native_field(&request, "use_color")?),
                (
                    "display_options",
                    new_record_with(
                        py,
                        RecordId::DisplayOptions,
                        &[("max_entries_per_section", limit)],
                    )?,
                ),
            ],
        )?),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    emit(
        &args.value("invocation")?.getattr("output_stream")?,
        &format!("{}\n\n", rendered.extract::<String>()?),
    )
}

/// Execute dbt without introducing any SQLBuild state callbacks.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn execute_dbt_without_state_tracking<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(py, "execute_dbt_without_state_tracking", options, OPTIONS)?;
    let request = args.value("request")?;
    let invocation = args.value("invocation")?;
    execute_dbt_commands(
        py,
        Some(keywords(
            py,
            &[
                ("runner", native_field(&invocation, "runner")?),
                ("options", native_field(&invocation, "dbt_options")?),
                ("merged_argv", args.value("merged_dbt_argv")?),
                (
                    "progress_stream",
                    native_field(&invocation, "output_stream")?,
                ),
                (
                    "stdout_stream",
                    native_field(&invocation, "dbt_output_stream")?,
                ),
                ("use_color", native_field(&request, "use_color")?),
                (
                    "skip_message",
                    text(py, "Skipping dbt: no dbt work selected."),
                ),
                ("on_progress", native_field(&request, "on_progress")?),
            ],
        )?),
    )
}

/// Write the summary of actual dbt execution events.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn write_dbt_execution_summary(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<()> {
    let args = KeywordArguments::read(py, "write_dbt_execution_summary", options, OPTIONS)?;
    let footer = render_dbt_execution_summary_footer(
        &args.value("execution")?.getattr("node_results")?,
        args.value("request")?
            .getattr("use_color")?
            .extract::<bool>()?,
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    if !footer.is_none() {
        emit(
            &args.value("invocation")?.getattr("output_stream")?,
            &format!("\n{}\n", footer.extract::<String>()?),
        )?;
    }
    Ok(())
}

/// Plan SQLBuild work after dbt using actual dbt failures as external blockers.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn resolve_sqlbuild_execution_plan_output<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "resolve_sqlbuild_execution_plan_output",
        options,
        OPTIONS,
    )?;
    let plan = args.value("plan")?;
    if !native_field(&plan, "sqlbuild_skip_reason")?.is_none() {
        return Ok(none(py));
    }
    let selected = native_field(&plan, "selection")?.getattr("sqlbuild_model_names")?;
    if !selected.is_truthy()? {
        return Ok(none(py));
    }
    let invocation = args.value("invocation")?;
    let stream = native_field(&invocation, "output_stream")?;
    emit(&stream, "\n")?;
    let reporter = ConnectionProgressReporter::new(
        py,
        Some(keywords(
            py,
            &[
                (
                    "adapter_name",
                    args.value("compiled")?.getattr("adapter_name")?,
                ),
                ("stream", stream),
                ("blank_line_after_complete", boolean(py, true)),
                ("use_color", args.value("request")?.getattr("use_color")?),
            ],
        )?),
    )
    .and_then(|value| pyo3::Bound::new(py, value))
    .map(pyo3::Bound::into_any)?;
    let request = new_record_with(
        py,
        RecordId::DbtSqlbuildPlanRequest,
        &[
            ("selected_model_names", selected),
            (
                "sqlbuild_args",
                native_field(&invocation, "effective_sqlbuild_args")?,
            ),
            (
                "external_blocked_model_names",
                args.value("failed_sqlbuild_model_names")?,
            ),
        ],
    )?;
    build_sqlbuild_plan_output(
        py,
        Some(keywords(
            py,
            &[
                ("environment", environment(&args)?),
                ("request", request),
                ("hooks", connection_hooks(&reporter, none(py))?),
            ],
        )?),
    )
}

fn execution_limits<'py>(
    discovered: &Bound<'py, PyAny>,
    target: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = discovered.py();
    if target.is_none() {
        return new_record(py, RecordId::ExecutionLimitsConfig, (), None);
    }
    resolve_target_config(
        py,
        &native_field(discovered, "project_config")?,
        &native_field(discovered, "local_config")?,
        &target,
    )?
    .getattr("execution_limits")
}

/// Execute the planned SQLBuild phase with the selected target's execution limits.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn run_dbt_sqlbuild_work<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(py, "run_dbt_sqlbuild_work", options, OPTIONS)?;
    let request = args.value("request")?;
    let invocation = args.value("invocation")?;
    let compiled = args.value("compiled")?;
    let project = native_field(&compiled, "project")?;
    let discovered = native_field(&invocation, "discovered_inputs")?;
    let context = new_record_with(
        py,
        RecordId::DbtSqlbuildWorkContext,
        &[
            ("plan_output", args.value("plan_output")?),
            ("connection_config", args.value("connection_config")?),
            ("adapter", native_field(&compiled, "adapter")?),
            ("adapter_name", native_field(&compiled, "adapter_name")?),
            ("output_stream", native_field(&invocation, "output_stream")?),
            ("use_color", native_field(&request, "use_color")?),
            (
                "snapshots_config",
                native_field(&discovered, "project_config")?.getattr("snapshots")?,
            ),
            (
                "execution_limits",
                execution_limits(
                    &discovered,
                    native_field(&project, "effective_target_name")?,
                )?,
            ),
            (
                "concurrency",
                resolve_concurrency(
                    &py.None().into_bound(py),
                    &discovered,
                    &native_field(&project, "effective_target_name")?,
                )?
                .limit
                .into_pyobject(py)?
                .into_any(),
            ),
        ],
    )?;
    execute_dbt_sqlbuild_work(
        py,
        Some(keywords(
            py,
            &[
                ("context", context),
                ("command", native_field(&request, "command")?),
                ("project", project),
                ("project_dir", native_field(&request, "project_dir")?),
                ("fail_fast", native_field(&request, "fail_fast")?),
                ("verbose", native_field(&request, "verbose")?),
            ],
        )?),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))
}

/// Report a SQLBuild skip transition without writing machine output to stdout.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn write_sqlbuild_skip_notice(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<()> {
    let args = KeywordArguments::read(py, "write_sqlbuild_skip_notice", options, OPTIONS)?;
    emit(&args.value("invocation")?.getattr("output_stream")?, "\n")?;
    progress(
        &args.value("request")?.getattr("on_progress")?,
        &args.value("message")?.extract::<String>()?,
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_dbt_connection_config, module)?)?;
    module.add_function(wrap_pyfunction!(write_dbt_execution_plan_text, module)?)?;
    module.add_function(wrap_pyfunction!(
        execute_dbt_without_state_tracking,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(write_dbt_execution_summary, module)?)?;
    module.add_function(wrap_pyfunction!(
        resolve_sqlbuild_execution_plan_output,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(run_dbt_sqlbuild_work, module)?)?;
    module.add_function(wrap_pyfunction!(write_sqlbuild_skip_notice, module)?)
}
