//! `sqb dag`: build the project graph and print its JSON or a one-line summary.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::query_command::main::render_dag_summary::render_dag_summary;

use crate::bindings::_helpers::dbt_interop::reference_resolver::resolve_external_sql_reference_resolver;
use crate::bindings::_helpers::query_command::project::{
    ProjectAdapter, ProjectInputs, project_adapter,
};
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_compile::compile_driver::main::setup::build_project_graph::build_project_graph;
use sqlbuild_python_compile::dag_artifact::main::build_dag_json::build_dag_json;
use sqlbuild_python_project::graph::main::python_nodes::python_node_graph::python_node_graph;

fn array_length(document: &serde_json::Value, key: &str) -> PyResult<usize> {
    document
        .get(key)
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .ok_or_else(|| PyValueError::new_err(format!("DAG JSON has no '{key}' list")))
}

/// Run one `sqb dag` invocation.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn run_dag_command<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<i32> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "run_dag_command",
        options,
        (
            &["project_dir"],
            &["no_sql_validation", "json_output", "cli_vars"],
        ),
    )?;
    let project_dir: Option<Bound<'py, PyAny>> = Some(arguments.value("project_dir")?);
    let no_sql_validation: bool = arguments.flag("no_sql_validation")?;
    let json_output: bool = arguments.flag("json_output")?;
    let cli_vars: Bound<'py, PyAny> = arguments.value("cli_vars")?;
    let ProjectAdapter {
        project_dir,
        discovered,
        adapter,
    } = project_adapter(py, project_dir, ProjectInputs::Discovered)?;
    let resolver: Bound<'py, PyAny> =
        resolve_external_sql_reference_resolver(project_dir, discovered.clone())?;
    let graph: Bound<'py, PyAny> = build_project_graph(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered.clone()),
                ("adapter", adapter),
                (
                    "no_sql_validation",
                    PyBool::new(py, no_sql_validation).to_owned().into_any(),
                ),
                ("cli_vars", cli_vars),
                ("external_sql_reference_resolver", resolver),
            ],
        )?),
    )?;
    let python_graph: Bound<'py, PyAny> = python_node_graph(&discovered)?;
    let dag_json: String = build_dag_json(
        &graph,
        &native_field(&native_field(&discovered, "project_config")?, "name")?
            .extract::<String>()?,
        Some(&python_graph),
    )?;
    let rendered: String = if json_output {
        format!("{dag_json}\n")
    } else {
        let document: serde_json::Value = serde_json::from_str(&dag_json)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        render_dag_summary((
            array_length(&document, "nodes")?,
            array_length(&document, "edges")?,
            array_length(&document, "checks")?,
        ))
    };
    py.import("sys")?
        .getattr("stdout")?
        .call_method1("write", (rendered,))?;
    Ok(0)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_dag_command, module)?)?;
    Ok(())
}
