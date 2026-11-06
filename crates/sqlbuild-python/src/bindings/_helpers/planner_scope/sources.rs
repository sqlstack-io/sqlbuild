//! Existing source and load plan models materialised at the Python boundary.

use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::compiled_views::source_reads;
use crate::bindings::_helpers::graph::keys::native_key;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::planner_scope::loaders::intermediate_map;
use crate::bindings::_helpers::planner_scope::values::{edges, keys};
use crate::bindings::_helpers::records::construct::new_record;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, pyfunction};
use pyo3::types::PyDictMethods;
use pyo3::types::{PyBool, PyDict, PyTuple};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::main::reachable_keys::reachable_keys;
use sqlbuild_analysis::planner_scope::constants::SOURCE;

#[pyfunction]
#[pyo3(signature = (*, project, selected_keys, upstream_deps = None))]
pub(crate) fn build_source_load_map<'py>(
    project: &Bound<'py, PyAny>,
    selected_keys: &Bound<'py, PyAny>,
    upstream_deps: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyDict>> {
    source_load_map(
        project,
        &keys(selected_keys)?,
        upstream_deps.map(edges).transpose()?.as_deref(),
    )
}

/// Select source entries from native reachability; the resulting entries cross the executor edge.
pub(crate) fn source_load_map<'py>(
    project: &Bound<'py, PyAny>,
    selected: &[ObjectKey],
    upstream_deps: Option<&[(ObjectKey, Vec<ObjectKey>)]>,
) -> PyResult<Bound<'py, PyDict>> {
    let relevant = upstream_deps.map(|edges| reachable_keys(selected, edges));
    let result = PyDict::new(project.py());
    for source in project_reads::sources(project)?.try_iter()? {
        let source = source?;
        if let Some(keys) = &relevant
            && !keys.contains(&native_key(&source_reads::key(&source)?)?)
        {
            continue;
        }
        let entry = source_reads::source_entry(&source)?;
        result.set_item(native_field(&entry, "name")?, entry)?;
    }
    for upstream in [false, true] {
        for (key, entry) in intermediate_map(project, selected, upstream)?.iter() {
            result.set_item(key, entry)?;
        }
    }
    Ok(result)
}

#[pyfunction]
#[pyo3(signature = (*, execution_order, selected_keys, source_map, is_reload))]
pub(crate) fn build_source_load_entries<'py>(
    execution_order: &Bound<'py, PyAny>,
    selected_keys: &Bound<'py, PyAny>,
    source_map: &Bound<'py, PyDict>,
    is_reload: bool,
) -> PyResult<Bound<'py, PyTuple>> {
    let py = execution_order.py();
    let kinds = host::entry(py, ExecutorHost::ExecutionResourceKind)?;
    let mut entries: Vec<Bound<'py, PyAny>> = Vec::new();
    for key in execution_order.try_iter()? {
        let key = key?;
        if !selected_keys.contains(&key)?
            || native_field(&key, "resource_type")?
                .str()?
                .extract::<String>()?
                != SOURCE
        {
            continue;
        }
        let Some(entry) = source_map.get_item(native_field(&key, "name")?)? else {
            continue;
        };
        if native_field(&entry, "loader")?.is_none() {
            continue;
        }
        let kwargs = PyDict::new(py);
        kwargs.set_item("key", key)?;
        for field in [
            "name",
            "loader",
            "write_strategy",
            "cursor_column",
            "unique_key",
        ] {
            kwargs.set_item(field, native_field(&entry, field)?)?;
        }
        let table = native_field(&entry, "table")?;
        kwargs.set_item(
            "destination",
            if table.is_truthy()? {
                table
            } else {
                native_field(&entry, "name")?
            },
        )?;
        let intermediate = native_field(&entry, "meta")?
            .call_method1("get", ("sqlbuild_loader_node",))?
            .is(PyBool::new(py, true));
        kwargs.set_item(
            "resource_kind",
            kinds.getattr(if intermediate { "LOADER" } else { "SOURCE" })?,
        )?;
        kwargs.set_item("is_reload", is_reload)?;
        let integration = native_field(&entry, "integration_loader")?;
        kwargs.set_item(
            "integration_kind",
            if integration.is_none() {
                py.None().into_bound(py)
            } else {
                native_field(&integration, "kind")?
            },
        )?;
        entries.push(new_record(
            py,
            RecordId::SourceLoadPlanEntry,
            (),
            Some(&kwargs),
        )?);
    }
    PyTuple::new(py, entries)
}
