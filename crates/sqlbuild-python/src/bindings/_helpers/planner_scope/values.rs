//! Python function identity and compiled graph conversion.

use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::compiled_views::source_reads;
use crate::bindings::_helpers::graph::keys::native_key;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::planner_scope::models::LoaderGraph;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;

pub(crate) fn keys(values: &Bound<'_, PyAny>) -> PyResult<Vec<ObjectKey>> {
    values
        .try_iter()?
        .map(|value| native_key(&value?))
        .collect()
}

pub(crate) fn edges(values: &Bound<'_, PyAny>) -> PyResult<Vec<(ObjectKey, Vec<ObjectKey>)>> {
    mapping_items(values)?
        .try_iter()?
        .map(|item| {
            let (key, deps): (Bound<'_, PyAny>, Bound<'_, PyAny>) = item?.extract()?;
            Ok((native_key(&key)?, keys(&deps)?))
        })
        .collect()
}

/// Resolve dependencies using the same Python dictionary lookup as the authored function boundary.
pub(crate) fn loader_names<'py>(loaders: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let names = PyDict::new(loaders.py());
    for loader in loaders.try_iter()? {
        let loader = loader?;
        names.set_item(
            native_field(&loader, "function")?,
            native_field(&loader, "name")?,
        )?;
    }
    Ok(names)
}

pub(crate) fn dependency_names(
    loader: &Bound<'_, PyAny>,
    names: &Bound<'_, PyDict>,
) -> PyResult<Vec<String>> {
    let mut dependencies: Vec<String> = Vec::new();
    for dependency in native_field(loader, "depends_on")?.try_iter()? {
        if let Some(name) = names.get_item(dependency?)? {
            dependencies.push(name.extract()?);
        }
    }
    Ok(dependencies)
}

pub(crate) fn loader_graph(loaders: &Bound<'_, PyAny>) -> PyResult<LoaderGraph> {
    let names = loader_names(loaders)?;
    let mut graph = LoaderGraph::default();
    for loader in loaders.try_iter()? {
        let loader = loader?;
        graph.loaders.insert(
            native_field(&loader, "name")?.extract()?,
            dependency_names(&loader, &names)?,
        );
    }
    Ok(graph)
}

pub(crate) fn project_graph(project: &Bound<'_, PyAny>) -> PyResult<LoaderGraph> {
    let mut graph = loader_graph(&native_field(project, "loader_functions")?)?;
    for source in project_reads::sources(project)?.try_iter()? {
        let entry = source_reads::source_entry(&source?)?;
        graph.sources.push((
            native_field(&entry, "name")?.extract()?,
            native_field(&entry, "loader")?.extract()?,
        ));
    }
    Ok(graph)
}
