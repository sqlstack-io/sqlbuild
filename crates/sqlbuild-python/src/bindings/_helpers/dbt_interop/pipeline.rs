//! Public project pipelines for dbt plan, execution and debug.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::dbt_cli::main::dbt_debug_args::dbt_debug_args;
use std::time::Instant;

use crate::bindings::_helpers::build_progress::progress_reporters::ConnectionProgressReporter;
use crate::bindings::_helpers::compile_driver::discovery::discover_project_inputs;
use crate::bindings::_helpers::compile_driver::pipeline_entries::plan_has_executable_work;
use crate::bindings::_helpers::dbt_interop::cli::resolve_dbt_config;
use crate::bindings::_helpers::dbt_interop::execution::build_failed_sqlbuild_model_names;
use crate::bindings::_helpers::dbt_interop::execution::build_merged_dbt_execution_argv;
use crate::bindings::_helpers::dbt_interop::pipeline_phases::resolve_dbt_connection_config;
use crate::bindings::_helpers::dbt_interop::pipeline_phases::run_dbt_sqlbuild_work;
use crate::bindings::_helpers::dbt_interop::pipeline_prologue::progress;
use crate::bindings::_helpers::dbt_interop::runner::DbtRunner;
use crate::bindings::_helpers::dbt_interop::runtime_config::config_overrides;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::build_command::main::boolean::boolean;
use sqlbuild_python_base::build_command::main::none::none;
use sqlbuild_python_base::build_command::main::text::text;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::registry::native_attr::native_attr;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::NativeAttr;
use sqlbuild_python_base::records::types::RecordId;

const PLAN_OPTIONS: (&[&str], &[&str]) = (
    &["project_dir", "args"],
    &[
        "dbt_runner",
        "dbt_executable",
        "sqlbuild_executable",
        "no_sql_validation",
        "on_progress",
        "progress_stream",
        "use_color",
    ],
);
const DEBUG_OPTIONS: (&[&str], &[&str]) = (
    &["project_dir", "args", "stdout_stream", "stderr_stream"],
    &["dbt_runner", "dbt_executable"],
);

fn native<'py>(
    py: Python<'py>,
    name: NativeAttr,
    pairs: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    call_with(&native_attr(py, name)?, pairs)
}

fn prepare<'py>(
    request: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py = request.py();
    let invocation =
        native_attr(py, NativeAttr::ResolveDbtExecutionInvocation)?.call1((request,))?;
    let manifest = call_with(
        &native_attr(py, NativeAttr::LoadCompiledDbtManifest)?,
        &[
            ("runner", native_field(&invocation, "runner")?),
            ("dbt_options", native_field(&invocation, "dbt_options")?),
            ("full_refresh", boolean(py, false)),
            ("on_progress", native_field(request, "on_progress")?),
        ],
    )?;
    let compiled = call_with(
        &native_attr(py, NativeAttr::CompileDbtInteropProject)?,
        &[
            ("project_dir", native_field(request, "project_dir")?),
            (
                "discovered_inputs",
                native_field(&invocation, "discovered_inputs")?,
            ),
            ("manifest", manifest.clone()),
            ("dbt_vars", native_field(&invocation, "dbt_vars")?),
            (
                "no_sql_validation",
                native_field(request, "no_sql_validation")?,
            ),
            ("on_progress", native_field(request, "on_progress")?),
        ],
    )?;
    Ok((invocation, manifest, compiled))
}

fn resolve_plan<'py>(
    request: &Bound<'py, PyAny>,
    invocation: &Bound<'py, PyAny>,
    compiled: &Bound<'py, PyAny>,
    manifest: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    native(
        request.py(),
        NativeAttr::ResolveDbtInteropPlan,
        &[
            ("command", native_field(request, "command")?),
            ("invocation", invocation.clone()),
            ("compiled", compiled.clone()),
            ("manifest", manifest.clone()),
            (
                "sqlbuild_executable",
                native_field(request, "sqlbuild_executable")?,
            ),
            ("on_progress", native_field(request, "on_progress")?),
        ],
    )
}

/// Execute dbt first and then the SQLBuild work selected by the combined graph.
#[pyfunction]
fn execute_dbt_interop_from_project(request: &Bound<'_, PyAny>) -> PyResult<i64> {
    let py = request.py();
    let command = native_field(request, "command")?;
    let kind = record_class(py, RecordId::DbtInteropCommand)?;
    if !command.eq(native_field(&kind, "RUN")?)? && !command.eq(native_field(&kind, "BUILD")?)? {
        return Err(PyErr::from_value(
            error_class(py, ErrorId::DbtInteropArgument)?.call1((format!(
                "unsupported dbt interop execution command: {}",
                command.str()?
            ),))?,
        ));
    }
    let (invocation, manifest, compiled) = prepare(request)?;
    let resolution = resolve_plan(request, &invocation, &compiled, &manifest)?;
    let plan = native_field(&resolution, "plan")?;
    let merged = build_merged_dbt_execution_argv(
        py,
        Some(keywords(
            py,
            &[
                ("command", command),
                ("options", native_field(&invocation, "dbt_options")?),
                (
                    "routed_args",
                    native_field(&invocation, "routed")?.getattr("dbt_args")?,
                ),
                ("plan", plan.clone()),
            ],
        )?),
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    call_with(
        &native_attr(py, NativeAttr::WriteDbtExecutionPlanText)?,
        &[
            ("request", request.clone()),
            ("invocation", invocation.clone()),
            ("plan", plan.clone()),
            ("merged_dbt_argv", merged.clone()),
        ],
    )?;
    let execution = call_with(
        &native_attr(py, NativeAttr::ExecuteDbtWithoutStateTracking)?,
        &[
            ("request", request.clone()),
            ("invocation", invocation.clone()),
            ("merged_dbt_argv", merged),
        ],
    )?;
    call_with(
        &native_attr(py, NativeAttr::WriteDbtExecutionSummary)?,
        &[
            ("request", request.clone()),
            ("invocation", invocation.clone()),
            ("execution", execution.clone()),
        ],
    )?;
    let failed = build_failed_sqlbuild_model_names(
        &native_field(&resolution, "graph")?
            .cast_into::<crate::bindings::_helpers::dbt_interop::graph::DbtCombinedGraph>()?,
        &manifest.clone(),
        &native_field(&execution, "node_results")?,
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    let code: i64 = native_field(&execution, "returncode")?.extract()?;
    if code != 0 && !native_field(&execution, "node_results")?.is_truthy()? {
        return Ok(code);
    }
    let output = call_with(
        &native_attr(py, NativeAttr::ResolveSqlbuildExecutionPlanOutput)?,
        &[
            ("request", request.clone()),
            ("invocation", invocation.clone()),
            ("compiled", compiled.clone()),
            ("manifest", manifest),
            ("plan", plan.clone()),
            ("failed_sqlbuild_model_names", failed),
        ],
    )?;
    let skip = if output.is_none() {
        if native_field(&plan, "sqlbuild_skip_reason")?.is_none() {
            return Ok(code);
        }
        Some("No SQLBuild work selected.")
    } else if !plan_has_executable_work(py, Some(keywords(py, &[("plan", output.clone())])?))
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?
        .is_truthy()?
    {
        Some("Skipping SQLBuild: no executable work selected.")
    } else {
        None
    };
    if let Some(message) = skip {
        call_with(
            &native_attr(py, NativeAttr::WriteSqlbuildSkipNotice)?,
            &[
                ("request", request.clone()),
                ("invocation", invocation),
                ("message", text(py, message)),
            ],
        )?;
        return Ok(code);
    }
    let connection = resolve_dbt_connection_config(
        py,
        Some(keywords(
            py,
            &[
                ("request", request.clone()),
                ("invocation", invocation.clone()),
                ("compiled", compiled.clone()),
            ],
        )?),
    )?;
    let result = run_dbt_sqlbuild_work(
        py,
        Some(keywords(
            py,
            &[
                ("request", request.clone()),
                ("invocation", invocation),
                ("compiled", compiled),
                ("plan_output", output),
                ("connection_config", connection),
            ],
        )?),
    )?;
    Ok(code.max(result.extract()?))
}

/// Build an interop plan from actual project files, dbt artifacts and warehouse facts.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn plan_dbt_interop_from_project<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(py, "plan_dbt_interop_from_project", options, PLAN_OPTIONS)?;
    let request = new_record_with(
        py,
        RecordId::DbtInteropExecutionRequest,
        &[
            (
                "command",
                enum_member(py, RecordId::DbtInteropCommand, "PLAN")?,
            ),
            ("project_dir", args.value("project_dir")?),
            ("args", args.value("args")?),
            ("dbt_runner", args.value("dbt_runner")?),
            ("dbt_executable", args.value("dbt_executable")?),
            (
                "sqlbuild_executable",
                if args.passed("sqlbuild_executable")? {
                    args.value("sqlbuild_executable")?
                } else {
                    text(py, "sqb")
                },
            ),
            (
                "no_sql_validation",
                args.value_or_false("no_sql_validation")?,
            ),
            ("on_progress", args.value("on_progress")?),
            ("progress_stream", args.value("progress_stream")?),
            ("use_color", args.value_or_false("use_color")?),
        ],
    )?;
    let (invocation, manifest, compiled) = prepare(&request)?;
    let stream = args.value("progress_stream")?;
    let connection = if stream.is_none() {
        none(py)
    } else {
        ConnectionProgressReporter::new(
            py,
            Some(keywords(
                py,
                &[
                    ("adapter_name", native_field(&compiled, "adapter_name")?),
                    ("stream", stream),
                    ("use_color", args.value_or_false("use_color")?),
                ],
            )?),
        )
        .and_then(|value| pyo3::Bound::new(py, value))
        .map(pyo3::Bound::into_any)?
    };
    let start = Instant::now();
    let resolution = resolve_plan(&request, &invocation, &compiled, &manifest)?;
    let plan = call_with(
        &native_attr(py, NativeAttr::AttachSqlbuildPlanOutput)?,
        &[
            ("plan", native_field(&resolution, "plan")?),
            ("project_dir", args.value("project_dir")?),
            (
                "discovered_inputs",
                native_field(&invocation, "discovered_inputs")?,
            ),
            ("compiled", compiled),
            (
                "sqlbuild_args",
                native_field(&invocation, "effective_sqlbuild_args")?,
            ),
            ("connection_progress", connection),
            ("on_progress", args.value("on_progress")?),
        ],
    )?;
    progress(
        &args.value("on_progress")?,
        &format!(
            "Generated dbt interop plan. ({:.2}s)",
            start.elapsed().as_secs_f64()
        ),
    )?;
    Ok(plan)
}

/// Run dbt debug using SQLBuild's resolved dbt configuration.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn debug_dbt_from_project(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<i64> {
    let args = KeywordArguments::read(py, "debug_dbt_from_project", options, DEBUG_OPTIONS)?;
    let discovered = discover_project_inputs(
        py,
        Some(keywords(
            py,
            &[("project_dir", args.value("project_dir")?)],
        )?),
    )?;
    let dbt_args: Vec<String> = args
        .value("args")?
        .try_iter()?
        .map(|arg| arg?.extract())
        .collect::<PyResult<_>>()?;
    let overrides = config_overrides(py, &dbt_args)?;
    let resolved = resolve_dbt_config(
        py,
        Some(keywords(
            py,
            &[
                ("project_root", args.value("project_dir")?),
                (
                    "config",
                    native_field(&discovered, "project_config")?.getattr("dbt")?,
                ),
                (
                    "local_config",
                    native_field(&discovered, "local_config")?.getattr("dbt")?,
                ),
                ("overrides", overrides),
                ("require_project_dir", boolean(py, true)),
            ],
        )?),
    )?;
    let dbt_options = new_record_with(
        py,
        RecordId::DbtCliOptions,
        &[
            ("project_dir", native_field(&resolved, "project_dir")?),
            ("profiles_dir", native_field(&resolved, "profiles_dir")?),
            ("target", native_field(&resolved, "target")?),
        ],
    )?;
    let mut runner = args.value("dbt_runner")?;
    if !runner.is_truthy()? {
        runner = pyo3::Bound::new(
            py,
            DbtRunner::new(
                args.value("dbt_executable")?.extract::<Option<String>>()?,
                None,
            ),
        )
        .map(pyo3::Bound::into_any)?;
    }
    let argv: Vec<String> = args.value("args")?.extract()?;
    let result = call_with(
        &native_field(&runner, "debug")?,
        &[
            ("options", dbt_options),
            ("args", PyTuple::new(py, dbt_debug_args(&argv))?.into_any()),
        ],
    )?;
    let stdout = args.value("stdout_stream")?;
    let stderr = args.value("stderr_stream")?;
    stdout.call_method1("write", (native_field(&result, "stdout")?,))?;
    stderr.call_method1("write", (native_field(&result, "stderr")?,))?;
    stdout.call_method0("flush")?;
    stderr.call_method0("flush")?;
    native_field(&result, "returncode")?.extract()
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(execute_dbt_interop_from_project, module)?)?;
    module.add_function(wrap_pyfunction!(plan_dbt_interop_from_project, module)?)?;
    module.add_function(wrap_pyfunction!(debug_dbt_from_project, module)?)
}
