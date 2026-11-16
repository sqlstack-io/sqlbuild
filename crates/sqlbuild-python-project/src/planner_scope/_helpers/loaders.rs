//! Loader dependency mapping and synthetic source boundary construction.

use crate::graph::main::project::edge_dict::edge_dict;
use crate::graph::models::KeyCache;
use crate::planner_scope::main::edges::edges;
use crate::planner_scope::main::keys::keys;
use crate::planner_scope::main::project_graph::project_graph;
use pyo3::prelude::{Bound, IntoPyObject, PyAny, PyAnyMethods, PyResult, pyfunction};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::planner_scope::main::{
    expand_loaders::expand_loaders, intermediate_sources::intermediate_sources,
};
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::contracts::main::loader_parts::loader_parts;
use std::collections::HashSet;

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
