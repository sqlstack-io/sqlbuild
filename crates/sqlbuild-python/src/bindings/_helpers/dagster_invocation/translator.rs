//! The default `SqlBuildDagsterTranslator` mappings its overridable methods call.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyBytes, PyDict, PyDictMethods, PyString, PyStringMethods, PyTuple};
use sqlbuild_core::dagster_invocation::constants::{DEFAULT_ASSET_KIND, DEFAULT_ASSET_NAME};
use sqlbuild_core::dagster_invocation::main::asset_description::asset_description;
use sqlbuild_core::dagster_invocation::main::dagster_name_segment::dagster_name_segment;
use sqlbuild_core::dagster_invocation::main::dagster_tag_key::dagster_tag_key;
use sqlbuild_core::dagster_invocation::main::fallback_asset_description::fallback_asset_description;
use sqlbuild_core::text::main::python_strip::python_strip;
use sqlbuild_host::entries::models::ThirdPartyHost;

use crate::bindings::_helpers::dagster_invocation::access::{get, value_or};
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;

const ABC_MODULE: &str = "collections.abc";
const BUILTINS_MODULE: &str = "builtins";
const KIND_TAG: &str = "sqlbuild/kind";
const DEFAULT_GROUP: &str = "sqlbuild";
const DEFAULT_CHECK_PART: &str = "check";
const CHECK_PART_SEPARATOR: &str = "__";
/// Node fields copied into asset metadata when present, in order.
const METADATA_KEYS: [&str; 12] = [
    "path",
    "target",
    "description",
    "sql",
    "columns",
    "column_lineage",
    "group",
    "language",
    "materialization_type",
    "return_kind",
    "loader",
    "meta",
];

/// `str(value)`.
fn text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(value.str()?.to_string_lossy().into_owned())
}

/// `get(key) or default` read lazily: `default` is not consulted when the value is truthy.
fn text_or_default<'py>(mapping: &Bound<'py, PyAny>, key: &str, default: &str) -> PyResult<String> {
    let value: Bound<'py, PyAny> = get(mapping, key)?;
    if value.is_truthy()? {
        return text(&value);
    }
    Ok(default.to_owned())
}

fn abc_instance(value: &Bound<'_, PyAny>, class: &str) -> PyResult<bool> {
    value.is_instance(&imported(value.py(), ABC_MODULE, class)?)
}

/// A source node's non-empty, non-text `meta.dagster.asset_key` sequence.
fn authored_asset_key<'py>(node: &Bound<'py, PyAny>) -> PyResult<Option<Bound<'py, PyAny>>> {
    let source_kind: String = host_entry(node.py(), ThirdPartyHost::SourceNodeKind)?.extract()?;
    if text(&get(node, "kind")?)? != source_kind {
        return Ok(None);
    }
    let meta: Bound<'py, PyAny> = get(node, "meta")?;
    if !abc_instance(&meta, "Mapping")? {
        return Ok(None);
    }
    let dagster: Bound<'py, PyAny> = get(&meta, "dagster")?;
    if !abc_instance(&dagster, "Mapping")? {
        return Ok(None);
    }
    let asset_key: Bound<'py, PyAny> = get(&dagster, "asset_key")?;
    if !abc_instance(&asset_key, "Sequence")?
        || asset_key.is_instance_of::<PyString>()
        || asset_key.is_instance_of::<PyBytes>()
        || !asset_key.is_truthy()?
    {
        return Ok(None);
    }
    Ok(Some(asset_key))
}

/// The asset key path of a node: its authored Dagster key, or its SQLBuild asset key.
#[pyfunction]
pub(crate) fn dagster_default_asset_key_path(node: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let parts: Bound<'_, PyAny> = match authored_asset_key(node)? {
        Some(parts) => parts,
        None => node.get_item("asset_key")?,
    };
    parts.try_iter()?.map(|part| text(&part?)).collect()
}

/// The node's group, project name or `sqlbuild`, with `-` read as `_`.
#[pyfunction]
pub(crate) fn dagster_default_group_name(node: &Bound<'_, PyAny>) -> PyResult<String> {
    let group: Bound<'_, PyAny> = get(node, "group")?;
    let chosen: String = if group.is_truthy()? {
        text(&group)?
    } else {
        text_or_default(node, "project_name", DEFAULT_GROUP)?
    };
    Ok(chosen.replace('-', "_"))
}

/// The node kind tag and one empty-valued tag per SQLBuild tag with a usable key.
#[pyfunction]
pub(crate) fn dagster_default_tags<'py>(node: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = node.py();
    let tags: Bound<'py, PyDict> = PyDict::new(py);
    tags.set_item(
        KIND_TAG,
        text(&value_or(node, "kind", PyString::new(py, "").into_any())?)?,
    )?;
    for tag in value_or(node, "tags", PyTuple::empty(py).into_any())?.try_iter()? {
        let key: String = dagster_tag_key(&text(&tag?)?);
        if !key.is_empty() {
            tags.set_item(key, "")?;
        }
    }
    Ok(tags)
}

/// The SQLBuild identity fields and every present node field Dagster shows as metadata.
#[pyfunction]
pub(crate) fn dagster_default_metadata<'py>(
    node: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let metadata: Bound<'py, PyDict> = PyDict::new(node.py());
    metadata.set_item("sqlbuild_id", get(node, "id")?)?;
    metadata.set_item("sqlbuild_name", get(node, "name")?)?;
    metadata.set_item("sqlbuild_kind", get(node, "kind")?)?;
    metadata.set_item("sqlbuild_selector", get(node, "name")?)?;
    for key in METADATA_KEYS {
        if node.contains(key)? {
            metadata.set_item(key, node.get_item(key)?)?;
        }
    }
    Ok(metadata)
}

/// The generated description of a node without one.
fn fallback_description(node: &Bound<'_, PyAny>) -> PyResult<String> {
    let kind: String = text_or_default(node, "kind", DEFAULT_ASSET_KIND)?;
    let name: String = text_or_default(node, "name", DEFAULT_ASSET_NAME)?;
    let target: Bound<'_, PyAny> = get(node, "target")?;
    let qualified_name: String = if abc_instance(&target, "Mapping")? {
        text_or_default(&target, "qualified_name", "")?
    } else {
        String::new()
    };
    let materialization: Bound<'_, PyAny> = get(node, "materialization_type")?;
    let materialization: Option<String> = if materialization.is_none() {
        None
    } else {
        Some(
            imported(node.py(), BUILTINS_MODULE, "format")?
                .call1((materialization,))?
                .str()?
                .to_string_lossy()
                .into_owned(),
        )
    };
    Ok(fallback_asset_description(
        &kind,
        &name,
        &qualified_name,
        materialization.as_deref(),
    ))
}

/// A string field, or `None` when the field is not text.
fn text_field(node: &Bound<'_, PyAny>, key: &str) -> PyResult<Option<String>> {
    let value: Bound<'_, PyAny> = get(node, key)?;
    match value.cast::<PyString>() {
        Ok(value) => Ok(Some(value.to_string_lossy().into_owned())),
        Err(_) => Ok(None),
    }
}

/// The node's description, or a generated one, followed by its SQL or source file.
#[pyfunction]
pub(crate) fn dagster_default_description(node: &Bound<'_, PyAny>) -> PyResult<String> {
    let description: Bound<'_, PyAny> = get(node, "description")?;
    let description: String = if description.is_truthy()? {
        text(&description)?
    } else {
        fallback_description(node)?
    };
    let sql: Option<String> = text_field(node, "sql")?;
    let path: Option<String> = if sql
        .as_deref()
        .is_some_and(|sql| !python_strip(sql).is_empty())
    {
        None
    } else {
        text_field(node, "path")?
    };
    Ok(asset_description(
        &description,
        sql.as_deref(),
        path.as_deref(),
    ))
}

/// `kind__name[__column or target]`, each part made Dagster-safe.
#[pyfunction]
pub(crate) fn dagster_default_check_name(check: &Bound<'_, PyAny>) -> PyResult<String> {
    let py: Python<'_> = check.py();
    let default = || PyString::new(py, DEFAULT_CHECK_PART).into_any();
    let mut parts: Vec<String> = vec![
        text(&value_or(check, "kind", default())?)?,
        text(&value_or(check, "name", default())?)?,
    ];
    for attached in ["attached_column_name", "attached_target_name"] {
        if !get(check, attached)?.is_none() {
            parts.push(text(&check.get_item(attached)?)?);
            break;
        }
    }
    Ok(parts
        .iter()
        .filter(|part| !part.is_empty())
        .map(|part| dagster_name_segment(part))
        .collect::<Vec<String>>()
        .join(CHECK_PART_SEPARATOR))
}

/// The SQLBuild identity fields of one check.
#[pyfunction]
pub(crate) fn dagster_default_check_metadata<'py>(
    check: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let metadata: Bound<'py, PyDict> = PyDict::new(check.py());
    metadata.set_item("sqlbuild_check_id", get(check, "id")?)?;
    metadata.set_item("sqlbuild_check_kind", get(check, "kind")?)?;
    metadata.set_item("sqlbuild_check_name", get(check, "name")?)?;
    metadata.set_item("sqlbuild_check_selector", get(check, "id")?)?;
    Ok(metadata)
}
