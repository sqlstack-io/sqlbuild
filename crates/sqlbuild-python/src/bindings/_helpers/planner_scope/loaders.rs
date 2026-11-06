//! Loader dependency mapping and synthetic source boundary construction.

use crate::bindings::_helpers::contracts::spec_resolution::loader_parts;
use crate::bindings::_helpers::graph::keys::KeyCache;
use crate::bindings::_helpers::graph::views::edge_dict;
use crate::bindings::_helpers::planner_scope::values::{
    dependency_names, edges, keys, loader_graph, loader_names, project_graph,
};
use crate::bindings::_helpers::records::construct::new_record;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use pyo3::prelude::{Bound, IntoPyObject, PyAny, PyAnyMethods, PyResult, pyfunction};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyTuple};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::planner_scope::main::{
    expand_loaders::expand_loaders, intermediate_sources::intermediate_sources,
};
use std::collections::HashSet;

#[pyfunction]
#[pyo3(signature = (*, loader_function, loader_functions))]
pub(crate) fn upstream_loader_dependency_names<'py>(
    loader_function: &Bound<'py, PyAny>,
    loader_functions: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let graph = loader_graph(loader_functions)?;
    let names = loader_names(loader_functions)?;
    let deps = dependency_names(loader_function, &names)?;
    PyTuple::new(loader_function.py(), graph.upstream(&deps))
}

pub(crate) fn loader_to_source_entry<'py>(
    project: &Bound<'py, PyAny>,
    loader: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = project.py();
    let name = native_field(loader, "name")?;
    let mut destination = native_field(loader, "destination")?;
    if destination.is_none() {
        destination = format!("__loader__{}", name.extract::<String>()?)
            .into_pyobject(py)?
            .into_any();
    }
    let parts = loader_parts(
        py,
        &destination.extract::<String>()?,
        (
            Some(native_field(project, "effective_target_database")?),
            Some(native_field(project, "effective_target_schema")?),
        ),
    )?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("name", &name)?;
    kwargs.set_item("loader", &name)?;
    for field in ["database", "schema", "table"] {
        kwargs.set_item(field, native_field(&parts, field)?)?;
    }
    for field in [
        "write_strategy",
        "cursor_column",
        "unique_key",
        "contract",
        "columns",
    ] {
        kwargs.set_item(field, native_field(loader, field)?)?;
    }
    let meta = PyDict::new(py);
    meta.set_item("sqlbuild_loader_node", true)?;
    kwargs.set_item("meta", meta)?;
    new_record(py, RecordId::SourceEntry, (), Some(&kwargs))
}

pub(crate) fn intermediate_map<'py>(
    project: &Bound<'py, PyAny>,
    selected: &[ObjectKey],
    upstream: bool,
) -> PyResult<Bound<'py, PyDict>> {
    let graph = project_graph(project)?;
    let names = intermediate_sources(&graph, selected, upstream);
    let loaders = PyDict::new(project.py());
    for loader in native_field(project, "loader_functions")?.try_iter()? {
        let loader = loader?;
        loaders.set_item(native_field(&loader, "name")?, &loader)?;
    }
    let result = PyDict::new(project.py());
    for name in names {
        if let Some(loader) = loaders.get_item(&name)? {
            result.set_item(&name, loader_to_source_entry(project, &loader)?)?;
        }
    }
    Ok(result)
}

#[pyfunction]
#[pyo3(signature = (*, project, selected_keys))]
pub(crate) fn build_intermediate_source_map<'py>(
    project: &Bound<'py, PyAny>,
    selected_keys: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    intermediate_map(project, &keys(selected_keys)?, false)
}

pub(crate) fn build_upstream_intermediate_source_map<'py>(
    project: &Bound<'py, PyAny>,
    selected_keys: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    intermediate_map(project, &keys(selected_keys)?, true)
}

#[pyfunction]
#[pyo3(signature = (*, project, selected_keys, upstream_deps, executable_dependency_source_keys = None))]
pub(crate) fn expand_selected_loader_dependencies<'py>(
    project: &Bound<'py, PyAny>,
    selected_keys: &Bound<'py, PyAny>,
    upstream_deps: &Bound<'py, PyAny>,
    executable_dependency_source_keys: Option<&Bound<'py, PyAny>>,
) -> PyResult<(Bound<'py, PyFrozenSet>, Bound<'py, PyDict>)> {
    let graph = project_graph(project)?;
    let selected = keys(selected_keys)?;
    let upstream = edges(upstream_deps)?;
    let executable = executable_dependency_source_keys
        .map(keys)
        .transpose()?
        .unwrap_or_default()
        .into_iter()
        .collect::<HashSet<_>>();
    let (selected, upstream) = expand_loaders(&graph, selected, upstream, &executable);
    let cache = KeyCache::new(project.py())?;
    Ok((
        PyFrozenSet::new(project.py(), cache.keys(&selected)?)?,
        edge_dict(&cache, &upstream)?,
    ))
}
