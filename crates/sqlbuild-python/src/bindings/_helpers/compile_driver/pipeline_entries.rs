//! Small pipeline entry points: analysis selection, project graphs and pipeline results.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyFloat};
use pyo3::{pyfunction, wrap_pyfunction};

use crate::bindings::_helpers::compile_driver::pipeline::compile_project_phase;
use crate::bindings::_helpers::graph::project_graph::{CompiledProjectGraph, NativeProjectGraph};
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// Discovered Python node functions whose selectors need the Python graph.
const PYTHON_NODE_FUNCTIONS: [&str; 4] = [
    "loader_functions",
    "task_functions",
    "asset_functions",
    "check_functions",
];
/// Plan entry lists that are runtime work.
const PLAN_WORK_ENTRIES: [&str; 6] = [
    "model_entries",
    "seed_entries",
    "source_load_entries",
    "function_entries",
    "audit_entries",
    "test_entries",
];

/// Limit analysis when selectors cannot resolve through Python graph nodes.
#[pyfunction]
#[pyo3(signature = (**options))]
fn resolve_compile_analysis_selection<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "resolve_compile_analysis_selection",
        options,
        (&["options", "discovered_inputs"], &[]),
    )?;
    compile_analysis_selection(
        &arguments.value("options")?,
        &arguments.value("discovered_inputs")?,
    )
}

/// `resolve_compile_analysis_selection` with its arguments.
pub(crate) fn compile_analysis_selection<'py>(
    options: &Bound<'py, PyAny>,
    discovered: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = options.py();
    let mut has_python_nodes: bool = false;
    for name in PYTHON_NODE_FUNCTIONS {
        if native_field(discovered, name)?.is_truthy()? {
            has_python_nodes = true;
            break;
        }
    }
    let no_cache: Bound<'py, PyAny> = native_field(options, "no_cache")?;
    if (native_field(options, "resolve_python_run_selectors")?.is_truthy()? && has_python_nodes)
        || native_field(options, "sql_test_selection")?
            .getattr("names")?
            .is_truthy()?
    {
        return new_record_with(
            py,
            RecordId::CompileAnalysisSelection,
            &[("no_cache", no_cache)],
        );
    }
    new_record_with(
        py,
        RecordId::CompileAnalysisSelection,
        &[
            ("select", native_field(options, "select")?),
            ("exclude", native_field(options, "exclude")?),
            (
                "auto_load_sources",
                native_field(options, "auto_load_sources")?,
            ),
            ("no_cache", no_cache),
        ],
    )
}

/// Build a static dependency graph for an already-compiled project.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn build_project_graph_from_compiled_project<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_project_graph_from_compiled_project",
        options,
        (&["project"], &[]),
    )?;
    project_graph(&arguments.value("project")?)
}

/// The project's natively held graph; dict views are built only when a boundary reads them.
pub(crate) fn project_graph<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let native: Bound<'py, NativeProjectGraph> =
        native_field(project, "lineage_graph")?.cast_into()?;
    Ok(Bound::new(
        project.py(),
        CompiledProjectGraph::new(project.clone().unbind(), native.unbind()),
    )?
    .into_any())
}

/// Return whether a planned build has runtime work to execute.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn plan_has_executable_work<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<bool> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "plan_has_executable_work",
        options,
        (&["plan"], &["python_plan_entries"]),
    )?;
    let plan: Bound<'py, PyAny> = arguments.value("plan")?;
    for name in PLAN_WORK_ENTRIES {
        if native_field(&plan, name)?.is_truthy()? {
            return Ok(true);
        }
    }
    if !arguments.passed("python_plan_entries")? {
        return Ok(false);
    }
    arguments.value("python_plan_entries")?.is_truthy()
}

/// Wrap a focused command projection in the established pipeline envelope.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn build_static_pipeline_result<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_static_pipeline_result",
        options,
        (&["context", "plan_output"], &[]),
    )?;
    let context: Bound<'py, PyAny> = arguments.value("context")?;
    new_record_with(
        py,
        RecordId::CompilePipelineResult,
        &[
            ("project", native_field(&context, "project")?),
            ("plan_output", arguments.value("plan_output")?),
            (
                "compile_seconds",
                native_field(&context, "compile_seconds")?,
            ),
            ("planning_seconds", PyFloat::new(py, 0.0).into_any()),
        ],
    )
}

/// Compile a project for consumers that construct their own focused plans.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn run_compile_only_pipeline<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "run_compile_only_pipeline",
        options,
        (
            &["discovered_inputs", "adapter", "options"],
            &["on_progress"],
        ),
    )?;
    let compiled: Bound<'py, PyAny> = compile_project_phase(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", arguments.value("discovered_inputs")?),
                ("adapter", arguments.value("adapter")?),
                ("options", arguments.value("options")?),
                ("on_progress", arguments.value("on_progress")?),
            ],
        )?),
    )?;
    new_record_with(
        py,
        RecordId::CompilePipelineResult,
        &[
            ("project", native_field(&compiled, "project")?),
            (
                "plan_output",
                new_record(py, RecordId::PlanOutput, (), None)?,
            ),
            (
                "compile_seconds",
                native_field(&compiled, "compile_seconds")?,
            ),
            ("planning_seconds", PyFloat::new(py, 0.0).into_any()),
        ],
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        resolve_compile_analysis_selection,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        build_project_graph_from_compiled_project,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(plan_has_executable_work, module)?)?;
    module.add_function(wrap_pyfunction!(build_static_pipeline_result, module)?)?;
    module.add_function(wrap_pyfunction!(run_compile_only_pipeline, module)?)
}
