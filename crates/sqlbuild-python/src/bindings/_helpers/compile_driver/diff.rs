//! The diff pipeline: both environments compiled, then diff selectors resolved to models.

use std::collections::HashSet;

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};

use sqlbuild_analysis::diff_selection::errors::SelectorError;
use sqlbuild_analysis::diff_selection::main::resolve_diff_selection::resolve_diff_selection;
use sqlbuild_analysis::diff_selection::models::DiffGraph;

use crate::bindings::_helpers::compile_driver::project::build_compiled_project;
use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::graph::views::project_lineage_views;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

/// Indexes of the `CompiledObjectKey`s a diff graph names, assigned on first sight.
struct KeyIndexes<'py> {
    indexes: Bound<'py, PyDict>,
}

impl<'py> KeyIndexes<'py> {
    fn index(&self, key: &Bound<'py, PyAny>) -> PyResult<usize> {
        if let Some(index) = self.indexes.get_item(key)? {
            return index.extract();
        }
        let index: usize = self.indexes.len();
        self.indexes.set_item(key, index)?;
        Ok(index)
    }

    fn indexes(&self, keys: &Bound<'py, PyAny>) -> PyResult<Vec<usize>> {
        keys.try_iter()?.map(|key| self.index(&key?)).collect()
    }
}

/// `edges` as one list of neighbour indexes per resource index.
fn adjacency<'py>(
    edges: &Bound<'py, PyAny>,
    keys: &KeyIndexes<'py>,
) -> PyResult<Vec<(usize, Vec<usize>)>> {
    let mut rows: Vec<(usize, Vec<usize>)> = Vec::new();
    for item in mapping_items(edges)?.try_iter()? {
        let item: Bound<'py, PyAny> = item?;
        rows.push((
            keys.index(&item.get_item(0)?)?,
            keys.indexes(&item.get_item(1)?)?,
        ));
    }
    Ok(rows)
}

/// The project's names, lineage edges and selector indexes over resource indexes.
fn diff_graph<'py>(project: &Bound<'py, PyAny>, keys: &KeyIndexes<'py>) -> PyResult<DiffGraph> {
    let py: Python<'py> = project.py();
    let lineage: Bound<'py, PyAny> = project_lineage_views(py, project)?;
    let mut graph: DiffGraph = DiffGraph::default();
    for attribute in ["models", "sources", "seeds"] {
        for resource in native_field(project, attribute)?.try_iter()? {
            let resource: Bound<'py, PyAny> = resource?;
            let name: String = native_field(&resource, "name")?.extract()?;
            graph
                .names
                .push((name, keys.index(&native_field(&resource, "key")?)?));
        }
    }
    let upstream: Vec<(usize, Vec<usize>)> =
        adjacency(&native_field(&lineage, "upstream_deps")?, keys)?;
    let downstream: Vec<(usize, Vec<usize>)> =
        adjacency(&native_field(&lineage, "downstream_deps")?, keys)?;
    for item in native_field(&lineage, "tag_index")?
        .call_method0("items")?
        .try_iter()?
    {
        let item: Bound<'py, PyAny> = item?;
        graph.tags.push((
            item.get_item(0)?.extract()?,
            keys.indexes(&item.get_item(1)?)?,
        ));
    }
    for item in native_field(&lineage, "path_index")?
        .call_method0("items")?
        .try_iter()?
    {
        let item: Bound<'py, PyAny> = item?;
        graph.paths.push((
            keys.index(&item.get_item(0)?)?,
            item.get_item(1)?.extract()?,
        ));
    }
    let size: usize = keys.indexes.len();
    graph.upstream = vec![Vec::new(); size];
    graph.downstream = vec![Vec::new(); size];
    for (key, neighbours) in upstream {
        graph.upstream[key] = neighbours;
    }
    for (key, neighbours) in downstream {
        graph.downstream[key] = neighbours;
    }
    Ok(graph)
}

/// The `PlannerInputError` a rejected selector raises.
fn selector_error(py: Python<'_>, error: SelectorError) -> PyErr {
    let raised: PyResult<Bound<'_, PyAny>> = (|| {
        error_class(py, ErrorId::PlannerInput)?.call(
            (error.message,),
            Some(&keywords(
                py,
                &[("code", PyString::new(py, error.code).into_any())],
            )?),
        )
    })();
    match raised {
        Ok(raised) => PyErr::from_value(raised),
        Err(raised) => raised,
    }
}

/// Resolve diff selectors to model names only.
#[pyfunction]
#[pyo3(signature = (**options))]
fn resolve_diff_model_names<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "resolve_diff_model_names",
        options,
        (&["project", "select", "exclude"], &[]),
    )?;
    diff_model_names(
        &arguments.value("project")?,
        &arguments.value("select")?,
        &arguments.value("exclude")?,
    )
}

/// `resolve_diff_model_names` with its arguments.
fn diff_model_names<'py>(
    project: &Bound<'py, PyAny>,
    select: &Bound<'py, PyAny>,
    exclude: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = project.py();
    let keys: KeyIndexes<'py> = KeyIndexes {
        indexes: PyDict::new(py),
    };
    let graph: DiffGraph = diff_graph(project, &keys)?;
    let select: Vec<String> = select.extract()?;
    let exclude: Vec<String> = exclude.extract()?;
    let selected: HashSet<usize> = resolve_diff_selection(&graph, (&select, &exclude))
        .map_err(|error| selector_error(py, error))?;
    let model_type: Bound<'py, PyAny> = enum_member(py, RecordId::CompiledResourceType, "MODEL")?;
    let mut names: Vec<Bound<'py, PyAny>> = Vec::new();
    for model in native_field(project, "models")?.try_iter()? {
        let model: Bound<'py, PyAny> = model?;
        let key: Bound<'py, PyAny> = model_reads::model_key(&model)?;
        let chosen: bool = match keys.indexes.get_item(&key)? {
            Some(index) => selected.contains(&index.extract::<usize>()?),
            None => false,
        };
        if chosen && native_field(&key, "resource_type")?.eq(&model_type)? {
            names.push(model_reads::name_object(&model)?);
        }
    }
    PyTuple::new(py, names)
}

/// Compile both environments and resolve selected diffable model names.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn run_diff_pipeline<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "run_diff_pipeline",
        options,
        (
            &[
                "discovered_inputs",
                "adapter",
                "from_target",
                "to_target",
                "resolved_connection",
                "no_sql_validation",
                "select",
                "exclude",
            ],
            &["cli_vars", "external_sql_reference_resolver"],
        ),
    )?;
    let compile = |target: &str| -> PyResult<Bound<'py, PyAny>> {
        build_compiled_project(
            py,
            Some(keywords(
                py,
                &[
                    ("discovered_inputs", arguments.value("discovered_inputs")?),
                    ("adapter", arguments.value("adapter")?),
                    ("selected_target", arguments.value(target)?),
                    (
                        "resolved_connection",
                        arguments.value("resolved_connection")?,
                    ),
                    ("no_sql_validation", arguments.value("no_sql_validation")?),
                    ("cli_vars", arguments.value("cli_vars")?),
                    (
                        "external_sql_reference_resolver",
                        arguments.value("external_sql_reference_resolver")?,
                    ),
                ],
            )?),
        )
    };
    let left: Bound<'py, PyAny> = compile("from_target")?;
    let right: Bound<'py, PyAny> = compile("to_target")?;
    let names: Bound<'py, PyTuple> = diff_model_names(
        &right,
        &arguments.value("select")?,
        &arguments.value("exclude")?,
    )?;
    PyTuple::new(py, [left, right, names.into_any()])
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_diff_model_names, module)?)?;
    module.add_function(wrap_pyfunction!(run_diff_pipeline, module)?)
}
