//! Discover, compile and resolve the combined dbt pipeline through its existing boundaries.

use crate::bindings::_helpers::compile_driver::discovery::discover_project_inputs;
use crate::bindings::_helpers::compile_driver::project::build_compiled_project;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_effective_adapter_name;
use crate::bindings::_helpers::dbt_interop::cli::resolve_dbt_executable;
use crate::bindings::_helpers::dbt_interop::cli::route_dbt_interop_args;
use crate::bindings::_helpers::dbt_interop::graph::build_dbt_combined_graph;
use crate::bindings::_helpers::dbt_interop::manifest::load_dbt_manifest_index;
use crate::bindings::_helpers::dbt_interop::orchestration::plan_dbt_interop_command;
use crate::bindings::_helpers::dbt_interop::plan_output::dbt_failure_detail;
use crate::bindings::_helpers::dbt_interop::runner::DbtRunner;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;

use crate::bindings::_helpers::dbt_interop::reference_resolver::DbtCompileReferenceResolver;
use crate::bindings::_helpers::dbt_interop::runtime_config::{
    interop_adapter, manifest_path, parse_dbt_execution_args, plan_options, vars_mapping,
};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use std::time::Instant;

const COMPILE_OPTIONS: (&[&str], &[&str]) = (
    &[
        "project_dir",
        "discovered_inputs",
        "manifest",
        "dbt_vars",
        "no_sql_validation",
        "on_progress",
    ],
    &[],
);
const MANIFEST_OPTIONS: (&[&str], &[&str]) = (
    &["runner", "dbt_options", "full_refresh", "on_progress"],
    &[],
);
const PLAN_OPTIONS: (&[&str], &[&str]) = (
    &[
        "command",
        "invocation",
        "compiled",
        "manifest",
        "sqlbuild_executable",
        "on_progress",
    ],
    &[],
);

pub(crate) fn progress(callback: &Bound<'_, PyAny>, message: &str) -> PyResult<()> {
    if !callback.is_none() {
        callback.call1((message,))?;
    }
    Ok(())
}

fn completed(callback: &Bound<'_, PyAny>, message: &str, start: Instant) -> PyResult<()> {
    progress(
        callback,
        &format!("{message} ({:.2}s)", start.elapsed().as_secs_f64()),
    )
}

/// Resolve the execution invocation without invoking dbt or SQLBuild work.
#[pyfunction]
pub(crate) fn resolve_dbt_execution_invocation<'py>(
    request: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = request.py();
    let mut executable = native_field(request, "dbt_executable")?;
    if !executable.is_truthy()? {
        executable = pyo3::IntoPyObjectExt::into_bound_py_any(resolve_dbt_executable(), py)?;
    }
    let mut stream = native_field(request, "progress_stream")?;
    if !stream.is_truthy()? {
        stream = py.import("sys")?.getattr("stdout")?;
    }
    let command = native_field(request, "command")?;
    let parsed = parse_dbt_execution_args(&command, &native_field(request, "args")?)?;
    let routed = route_dbt_interop_args(py, &command, &parsed)?;
    let project_dir = native_field(request, "project_dir")?;
    let discovered = discover_project_inputs(
        py,
        Some(keywords(py, &[("project_dir", project_dir.clone())])?),
    )?;
    let dbt_args: Vec<String> = native_field(&routed, "dbt_args")?
        .try_iter()?
        .map(|arg| arg?.extract())
        .collect::<PyResult<_>>()?;
    let options = plan_options(&project_dir, &discovered, &dbt_args)?;
    let vars = vars_mapping(
        &native_field(&discovered, "project_config")?.getattr("dbt")?,
        &native_field(&discovered, "local_config")?.getattr("dbt")?,
        &dbt_args,
    )?
    .into_any();
    let mut runner = native_field(request, "dbt_runner")?;
    if !runner.is_truthy()? {
        runner = pyo3::Bound::new(
            py,
            DbtRunner::new(executable.clone().extract::<Option<String>>()?, None),
        )
        .map(pyo3::Bound::into_any)?;
    }
    let dbt_stream = native_field(request, "dbt_stdout_stream")?;
    new_record_with(
        py,
        RecordId::DbtInteropInvocation,
        &[
            ("dbt_executable", executable),
            ("output_stream", stream.clone()),
            (
                "dbt_output_stream",
                if dbt_stream.is_truthy()? {
                    dbt_stream
                } else {
                    stream
                },
            ),
            (
                "effective_sqlbuild_args",
                native_field(&routed, "sqlbuild_args")?,
            ),
            ("routed", routed),
            ("discovered_inputs", discovered),
            ("dbt_options", options),
            ("dbt_vars", vars),
            ("runner", runner),
        ],
    )
}

/// Compile dbt and load the manifest after a successful subprocess result.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn load_compiled_dbt_manifest<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(py, "load_compiled_dbt_manifest", options, MANIFEST_OPTIONS)?;
    let callback = args.value("on_progress")?;
    let start = Instant::now();
    progress(&callback, "Compiling dbt project...")?;
    let result = call_with(
        &args.value("runner")?.getattr("compile")?,
        &[
            ("options", args.value("dbt_options")?),
            ("full_refresh", args.value("full_refresh")?),
        ],
    )?;
    if native_field(&result, "returncode")?.extract::<i64>()? != 0 {
        let detail = dbt_failure_detail(&result)?;
        let error = error_class(py, ErrorId::DbtInteropRuntime)?.call(
            ("dbt compile failed",),
            Some(&keywords(py, &[("help", detail)])?),
        )?;
        return Err(PyErr::from_value(error));
    }
    completed(&callback, "Compiled dbt project.", start)?;
    let start = Instant::now();
    progress(&callback, "Loading dbt manifest...")?;
    let path = manifest_path(&args.value("dbt_options")?)?;
    let manifest = load_dbt_manifest_index(py, path)?;
    completed(&callback, "Loaded dbt manifest.", start)?;
    Ok(manifest)
}

/// Compile SQLBuild models against dbt references using the existing adapter boundary.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn compile_dbt_interop_project<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(py, "compile_dbt_interop_project", options, COMPILE_OPTIONS)?;
    let callback = args.value("on_progress")?;
    let start = Instant::now();
    progress(&callback, "Compiling SQLBuild project...")?;
    let discovered = args.value("discovered_inputs")?;
    let name = resolve_effective_adapter_name(
        &native_field(&discovered, "project_config")?,
        &native_field(&discovered, "local_config")?,
    )?;
    let adapter = interop_adapter(&name, &args.value("project_dir")?)?;
    let resolver = Bound::new(
        py,
        DbtCompileReferenceResolver::new(Some(args.value("manifest")?.unbind())),
    )?
    .into_any();
    let project = build_compiled_project(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered),
                ("adapter", adapter.clone()),
                ("no_sql_validation", args.value("no_sql_validation")?),
                ("cli_vars", args.value("dbt_vars")?),
                ("external_sql_reference_resolver", resolver),
            ],
        )?),
    )?;
    completed(&callback, "Compiled SQLBuild project.", start)?;
    new_record_with(
        py,
        RecordId::DbtInteropCompiledProject,
        &[
            ("adapter_name", name),
            ("adapter", adapter),
            ("project", project),
        ],
    )
}

/// Build the combined graph and resolve its dbt and SQLBuild selections.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn resolve_dbt_interop_plan<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(py, "resolve_dbt_interop_plan", options, PLAN_OPTIONS)?;
    let callback = args.value("on_progress")?;
    let start = Instant::now();
    progress(&callback, "Building dbt interop graph...")?;
    let project = args.value("compiled")?.getattr("project")?;
    let graph = build_dbt_combined_graph(py, args.value("manifest")?, project.clone())
        .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    completed(&callback, "Built dbt interop graph.", start)?;
    let start = Instant::now();
    progress(&callback, "Resolving dbt and SQLBuild selection...")?;
    let invocation = args.value("invocation")?;
    let routed = native_field(&invocation, "routed")?;
    let tuple = py.import("builtins")?.getattr("tuple")?;
    let command_args = new_record_with(
        py,
        RecordId::DbtInteropCommandArgs,
        &[
            ("select", tuple.call1((native_field(&routed, "select")?,))?),
            (
                "exclude",
                tuple.call1((native_field(&routed, "exclude")?,))?,
            ),
            (
                "dbt_command_args",
                tuple.call1((native_field(&routed, "dbt_args")?,))?,
            ),
            (
                "sqlbuild_command_args",
                tuple.call1((native_field(&invocation, "effective_sqlbuild_args")?,))?,
            ),
            (
                "dbt_executable",
                native_field(&invocation, "dbt_executable")?,
            ),
            ("sqlbuild_executable", args.value("sqlbuild_executable")?),
        ],
    )?;
    let plan = plan_dbt_interop_command(
        py,
        Some(keywords(
            py,
            &[
                ("command", args.value("command")?),
                ("project", project),
                ("manifest", args.value("manifest")?),
                ("graph", graph.clone()),
                ("dbt_runner", native_field(&invocation, "runner")?),
                ("dbt_options", native_field(&invocation, "dbt_options")?),
                ("command_args", command_args),
            ],
        )?),
    )?;
    completed(&callback, "Resolved dbt and SQLBuild selection.", start)?;
    new_record_with(
        py,
        RecordId::DbtInteropPlanResolution,
        &[("graph", graph), ("plan", plan)],
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_dbt_execution_invocation, module)?)?;
    module.add_function(wrap_pyfunction!(load_compiled_dbt_manifest, module)?)?;
    module.add_function(wrap_pyfunction!(compile_dbt_interop_project, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_dbt_interop_plan, module)?)
}
