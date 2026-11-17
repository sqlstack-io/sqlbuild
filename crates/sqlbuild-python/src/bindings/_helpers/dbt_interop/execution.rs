//! dbt execution decisions: merged argv, the counting `dbt ls`, failed models and the footer.

use std::collections::HashMap;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::dbt_cli::main::build_dbt_argv::build_dbt_argv;
use sqlbuild_core::dbt_cli::models::DbtArgvCommand;
use sqlbuild_core::dbt_graph::main::dbt_downstream_sqlbuild_models::dbt_downstream_sqlbuild_models;
use sqlbuild_core::dbt_plan::main::dbt_execution_ls_argv::dbt_execution_ls_argv as ls_argv;
use sqlbuild_core::dbt_plan::main::dbt_failed_unique_ids::dbt_failed_unique_ids;
use sqlbuild_core::dbt_plan::main::merged_dbt_execution_args::merged_dbt_execution_args;
use sqlbuild_core::dbt_plan::main::render_dbt_execution_summary::render_dbt_execution_summary;
use sqlbuild_core::dbt_plan::models::DbtExecutedNode;

use crate::bindings::_helpers::dbt_interop::cli::option_values;
use crate::bindings::_helpers::dbt_interop::graph::{DbtCombinedGraph, texts};
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;

const MERGED_OPTIONS: (&[&str], &[&str]) = (&["command", "options", "routed_args", "plan"], &[]);

/// Merge the original dbt selection with dbt models SQLBuild work requires; `None` without dbt work.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn build_merged_dbt_execution_argv<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Option<Bound<'py, PyTuple>>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_merged_dbt_execution_argv",
        options,
        MERGED_OPTIONS,
    )?;
    let plan: Bound<'py, PyAny> = arguments.value("plan")?;
    let required_terms: Vec<String> = texts(&native_field(&plan, "dbt_required_selector_terms")?)?;
    if !native_field(&plan, "dbt_selected_unique_ids")?.is_truthy()? && required_terms.is_empty() {
        return Ok(None);
    }
    let args: Vec<String> =
        merged_dbt_execution_args(&texts(&arguments.value("routed_args")?)?, &required_terms);
    let dbt_executable: String = native_field(&plan, "dbt_command_argv")?
        .get_item(0)?
        .extract()?;
    let command: DbtArgvCommand = DbtArgvCommand::Other {
        command: arguments.value("command")?.getattr("value")?.extract()?,
        args,
    };
    let options = option_values(&arguments.value("options")?)?;
    Ok(Some(PyTuple::new(
        py,
        build_dbt_argv(&dbt_executable, &options, command),
    )?))
}

/// The `dbt ls --output json` that counts an execution argv's selection, when it has a command.
#[pyfunction]
#[pyo3(signature = (*, argv))]
pub(crate) fn dbt_execution_ls_argv(argv: Vec<String>) -> Option<Vec<String>> {
    ls_argv(&argv)
}

fn executed_nodes(node_results: &Bound<'_, PyAny>) -> PyResult<Vec<DbtExecutedNode>> {
    let mut nodes: Vec<DbtExecutedNode> = Vec::new();
    for result in node_results.try_iter()? {
        let result: Bound<'_, PyAny> = result?;
        nodes.push(DbtExecutedNode {
            unique_id: native_field(&result, "unique_id")?.extract()?,
            resource_type: native_field(&result, "resource_type")?.extract()?,
            status: native_field(&result, "status")?.extract()?,
            execution_time: native_field(&result, "execution_time")?.extract()?,
        });
    }
    Ok(nodes)
}

/// SQLBuild models downstream of failed or skipped dbt models and the models failed tests check.
#[pyfunction]
#[pyo3(signature = (*, graph, manifest, node_results))]
pub(crate) fn build_failed_sqlbuild_model_names<'py>(
    graph: &Bound<'py, DbtCombinedGraph>,
    manifest: &Bound<'py, PyAny>,
    node_results: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let mut dependencies: HashMap<String, Vec<String>> = HashMap::new();
    let lookup: Bound<'py, PyAny> =
        native_field(manifest, "validation_depends_on_nodes_by_unique_id")?;
    for item in mapping_items(&lookup)?.try_iter()? {
        let (unique_id, depends_on): (String, Bound<'py, PyAny>) = item?.extract()?;
        dependencies.insert(unique_id, texts(&depends_on)?);
    }
    let failed: Vec<String> = dbt_failed_unique_ids(&executed_nodes(node_results)?, &dependencies);
    PyTuple::new(
        graph.py(),
        dbt_downstream_sqlbuild_models(graph.get().native(), &failed),
    )
}

/// Render a PASS/WARN/FAIL/SKIP/TOTAL footer for executed dbt nodes.
#[pyfunction]
#[pyo3(signature = (*, node_results, use_color))]
pub(crate) fn render_dbt_execution_summary_footer(
    node_results: &Bound<'_, PyAny>,
    use_color: bool,
) -> PyResult<Option<String>> {
    Ok(render_dbt_execution_summary(
        &executed_nodes(node_results)?,
        CliStyle { use_color },
    ))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_merged_dbt_execution_argv, module)?)?;
    module.add_function(wrap_pyfunction!(dbt_execution_ls_argv, module)?)?;
    module.add_function(wrap_pyfunction!(build_failed_sqlbuild_model_names, module)?)?;
    module.add_function(wrap_pyfunction!(
        render_dbt_execution_summary_footer,
        module
    )?)
}
