//! Reads of DAG artifact and execution document mappings, as the Dagster integration reads them.

use std::collections::HashSet;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyTuple};
use sqlbuild_host::entries::models::ThirdPartyHost;

use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;

/// An asset path as Dagster spells it.
pub(crate) type AssetPath = Vec<String>;
/// An asset path and one of its check names.
pub(crate) type CheckKey = (AssetPath, String);

/// `mapping.get(key)`.
pub(crate) fn get<'py>(mapping: &Bound<'py, PyAny>, key: &str) -> PyResult<Bound<'py, PyAny>> {
    mapping_get(mapping, key)
}

/// `mapping.get(key, default)`.
pub(crate) fn value_or<'py>(
    mapping: &Bound<'py, PyAny>,
    key: &str,
    default: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    mapping_get_or(mapping, key, default)
}

/// `str(mapping.get(key))`.
pub(crate) fn text_of(mapping: &Bound<'_, PyAny>, key: &str) -> PyResult<String> {
    get(mapping, key)?.str()?.extract()
}

/// The items of `mapping.get(key, ())`.
pub(crate) fn items<'py>(
    mapping: &Bound<'py, PyAny>,
    key: &str,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = mapping.py();
    value_or(mapping, key, PyTuple::empty(py).into_any())?
        .try_iter()?
        .collect()
}

/// `[str(part) for part in node["asset_key"]]`.
pub(crate) fn asset_path(node: &Bound<'_, PyAny>) -> PyResult<AssetPath> {
    node.get_item("asset_key")?
        .try_iter()?
        .map(|part| part?.str()?.extract())
        .collect()
}

/// `[str(part) for part in node.get("asset_key", ())]`.
pub(crate) fn asset_path_or_empty(node: &Bound<'_, PyAny>) -> PyResult<AssetPath> {
    items(node, "asset_key")?
        .iter()
        .map(|part| part.str()?.extract())
        .collect()
}

/// A Dagster integration constant.
pub(crate) fn constant<'py>(py: Python<'py>, name: ThirdPartyHost) -> PyResult<Bound<'py, PyAny>> {
    host_entry(py, name)
}

/// Whether a constant set holds `value`.
pub(crate) fn constant_contains(
    py: Python<'_>,
    (name, value): (ThirdPartyHost, &str),
) -> PyResult<bool> {
    constant(py, name)?.contains(value)
}

/// `bool(record.get(<constant field>, True))`.
pub(crate) fn enabled(record: &Bound<'_, PyAny>, field_constant: ThirdPartyHost) -> PyResult<bool> {
    let py: Python<'_> = record.py();
    let field: Bound<'_, PyAny> = constant(py, field_constant)?;
    mapping_get_or(record, field, true)?.is_truthy()
}

/// Whether a translated DAG node is owned by the SQLBuild asset definition.
pub(crate) fn asset_enabled(node: &Bound<'_, PyAny>) -> PyResult<bool> {
    enabled(node, ThirdPartyHost::DagsterAssetEnabledField)
}

/// Whether a translated DAG check is owned by the SQLBuild asset definition.
pub(crate) fn check_enabled(check: &Bound<'_, PyAny>) -> PyResult<bool> {
    enabled(check, ThirdPartyHost::DagsterCheckEnabledField)
}

/// `{str(node.get("id")): node for node in dag.get("nodes", ())}`, keeping the last node per id.
pub(crate) fn nodes_by_id<'py>(
    dag: &Bound<'py, PyAny>,
) -> PyResult<Vec<(String, Bound<'py, PyAny>)>> {
    let mut nodes: Vec<(String, Bound<'py, PyAny>)> = Vec::new();
    for node in items(dag, "nodes")? {
        let id: String = text_of(&node, "id")?;
        match nodes.iter_mut().find(|(listed, _)| *listed == id) {
            Some(entry) => entry.1 = node,
            None => nodes.push((id, node)),
        }
    }
    Ok(nodes)
}

/// The node with `id`, if any.
pub(crate) fn node_with_id<'a, 'py>(
    nodes: &'a [(String, Bound<'py, PyAny>)],
    id: &str,
) -> Option<&'a Bound<'py, PyAny>> {
    nodes
        .iter()
        .find(|(listed, _)| listed == id)
        .map(|(_, node)| node)
}

/// Selected asset paths passed from the Dagster context.
pub(crate) fn path_set(values: &Bound<'_, PyAny>) -> PyResult<HashSet<AssetPath>> {
    if values.is_none() {
        return Ok(HashSet::new());
    }
    values.try_iter()?.map(|path| path?.extract()).collect()
}

/// Selected asset check keys passed from the Dagster context.
pub(crate) fn check_key_set(values: &Bound<'_, PyAny>) -> PyResult<HashSet<CheckKey>> {
    if values.is_none() {
        return Ok(HashSet::new());
    }
    values.try_iter()?.map(|key| key?.extract()).collect()
}

/// A Python list of strings.
pub(crate) fn string_list<'py>(py: Python<'py>, values: &[String]) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyList::new(py, values)?.into_any())
}

/// A Python tuple of strings.
pub(crate) fn string_tuple<'py>(py: Python<'py>, values: &[String]) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyTuple::new(py, values)?.into_any())
}

/// `{**base, **extra}`, keeping each key's first position.
pub(crate) fn merged<'py>(
    base: &[(&str, Bound<'py, PyAny>)],
    extra: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = extra.py();
    let merged: Bound<'py, PyDict> = PyDict::new(py);
    for (key, value) in base {
        merged.set_item(*key, value)?;
    }
    for (key, value) in extra.iter() {
        merged.set_item(key, value)?;
    }
    Ok(merged)
}
