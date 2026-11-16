//! Python function identity and compiled graph conversion.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use sqlbuild_analysis::planner_scope::models::LoaderGraph;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

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
