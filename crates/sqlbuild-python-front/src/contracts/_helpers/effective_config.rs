//! Effective target, vars, connection and settings precedence over the loaded config objects.

use crate::contracts::main::effective_connection_fields::effective_connection_fields;
use std::hash::{Hash, Hasher};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, PyStringMethods, Python};
use pyo3::pyfunction;
use pyo3::types::{PyBytes, PyBytesMethods, PyDict, PyDictMethods, PyListMethods, PyString};
use sqlbuild_config::effective::constants::CONNECTION_KIND_KEY;
use sqlbuild_config::effective::errors::{EffectiveConfigError, MessagePart};
use sqlbuild_config::effective::main::default_connection_name::default_connection_name;
use sqlbuild_config::effective::main::effective_target_name::effective_target_name;
use sqlbuild_config::effective::main::merged_target::merged_target;
use sqlbuild_config::effective::main::target_fields::target_fields;
use sqlbuild_config::effective::models::{
    ConnectionChoice, FieldValue, MergeRule, TargetChoice, TargetField, TargetLayer,
};
use sqlbuild_config::effective::types::Entries;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const UTF8: &str = "utf-8";
const SURROGATE_PASS: &str = "surrogatepass";

/// A `str` key compared exactly: by its UTF-8 bytes, lone surrogates passed through.
#[derive(Clone)]
pub(crate) struct Key<'py> {
    bytes: Vec<u8>,
    handle: Bound<'py, PyString>,
}

impl Key<'_> {
    /// The key as text, lone surrogates replaced, for messages and fixed names.
    pub(crate) fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes).into_owned()
    }
}

impl PartialEq for Key<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.bytes == other.bytes
    }
}

impl Eq for Key<'_> {}

impl Hash for Key<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.bytes.hash(state);
    }
}

impl PartialOrd for Key<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Key<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.bytes.cmp(&other.bytes)
    }
}

pub(crate) type Value<'py> = Bound<'py, PyAny>;
type Layer<'py> = TargetLayer<Key<'py>, Value<'py>>;
pub(crate) type Pairs<'py> = Entries<Key<'py>, Value<'py>>;
/// `(group, name, value)` of each merged `TargetConfig` field.
type FieldRow<'py> = (Option<&'static str>, &'static str, Value<'py>);
/// A result, else the precedence failure message.
pub(crate) type Outcome<'py, T> = (Option<T>, Option<Bound<'py, PyString>>);

pub(crate) fn key<'py>(value: &Bound<'py, PyAny>) -> PyResult<Key<'py>> {
    let handle: Bound<'py, PyString> = value.cast::<PyString>()?.clone();
    if let Ok(text) = handle.to_str() {
        return Ok(Key {
            bytes: text.as_bytes().to_vec(),
            handle,
        });
    }
    let bytes: Vec<u8> = handle
        .call_method1("encode", (UTF8, SURROGATE_PASS))?
        .cast_into::<PyBytes>()?
        .as_bytes()
        .to_vec();
    Ok(Key { bytes, handle })
}

pub(crate) fn optional_key<'py>(value: &Bound<'py, PyAny>) -> PyResult<Option<Key<'py>>> {
    if value.is_none() {
        return Ok(None);
    }
    key(value).map(Some)
}

pub(crate) fn pairs<'py>(mapping: &Bound<'py, PyAny>) -> PyResult<Pairs<'py>> {
    mapping
        .cast::<PyDict>()?
        .iter()
        .map(|(name, value)| Ok((key(&name)?, value)))
        .collect()
}

fn keys<'py>(mapping: &Bound<'py, PyAny>) -> PyResult<Vec<Key<'py>>> {
    mapping
        .cast::<PyDict>()?
        .keys()
        .iter()
        .map(|name| key(&name))
        .collect()
}

/// The names in `connections` whose entry sets no `kind` in either file: the connections a target may default to.
fn kindless_keys<'py>(
    connections: &Bound<'py, PyAny>,
    other: &Bound<'py, PyAny>,
) -> PyResult<Vec<Key<'py>>> {
    let other: &Bound<'py, PyDict> = other.cast::<PyDict>()?;
    let mut names: Vec<Key<'py>> = Vec::new();
    for (name, connection) in connections.cast::<PyDict>()?.iter() {
        let sets_kind = |entry: &Bound<'py, PyAny>| -> PyResult<bool> {
            entry.cast::<PyDict>()?.contains(CONNECTION_KIND_KEY)
        };
        let elsewhere: Option<Bound<'py, PyAny>> = other.get_item(&name)?;
        if !sets_kind(&connection)?
            && !elsewhere
                .as_ref()
                .map(sets_kind)
                .transpose()?
                .unwrap_or(false)
        {
            names.push(key(&name)?);
        }
    }
    Ok(names)
}

pub(crate) fn dictionary<'py>(
    py: Python<'py>,
    entries: Pairs<'py>,
) -> PyResult<Bound<'py, PyDict>> {
    let dictionary: Bound<'py, PyDict> = PyDict::new(py);
    for (name, value) in entries {
        dictionary.set_item(name.handle, value)?;
    }
    Ok(dictionary)
}

fn field_owner<'py>(
    target: &Bound<'py, PyAny>,
    field: &TargetField,
) -> PyResult<Bound<'py, PyAny>> {
    match field.group {
        Some(group) => native_field(target, group),
        None => Ok(target.clone()),
    }
}

fn field_value<'py>(
    target: &Bound<'py, PyAny>,
    field: &TargetField,
) -> PyResult<FieldValue<Key<'py>, Value<'py>>> {
    let value: Bound<'py, PyAny> = native_field(&field_owner(target, field)?, field.name)?;
    match (field.rule, value.is_none()) {
        (MergeRule::Overlay { .. }, true) => Ok(FieldValue::Scalar(None)),
        (MergeRule::Overlay { .. }, false) => Ok(FieldValue::Scalar(Some(value))),
        (_, true) => Ok(FieldValue::Map(None)),
        (_, false) => Ok(FieldValue::Map(Some(pairs(&value)?))),
    }
}

/// One target dataclass as a layer of `TARGET_FIELDS`.
fn target_layer<'py>(target: &Bound<'py, PyAny>) -> PyResult<Layer<'py>> {
    Ok(TargetLayer {
        fields: target_fields()
            .iter()
            .map(|field| field_value(target, field))
            .collect::<PyResult<_>>()?,
    })
}

fn field_position(name: &str) -> usize {
    target_fields()
        .iter()
        .position(|field| field.group.is_none() && field.name == name)
        .unwrap_or_default()
}

fn is_set(value: &FieldValue<Key<'_>, Value<'_>>) -> bool {
    match value {
        FieldValue::Scalar(value) => value.is_some(),
        FieldValue::Map(value) => value.as_ref().is_some_and(|entries| !entries.is_empty()),
    }
}

/// The failure message as a Python `str`, authored names inserted unchanged.
pub(crate) fn message<'py>(
    py: Python<'py>,
    error: &EffectiveConfigError<Key<'py>>,
) -> PyResult<Bound<'py, PyString>> {
    let parts: Vec<Bound<'py, PyString>> = error
        .message_parts()
        .into_iter()
        .map(|part| match part {
            MessagePart::Text(text) => PyString::new(py, text),
            MessagePart::Name(name) => name.handle.clone(),
        })
        .collect();
    Ok(PyString::new(py, "")
        .call_method1("join", (parts,))?
        .cast_into::<PyString>()?)
}

/// `resolve_target_name`: the effective target, `None`, or the unknown-target message.
#[pyfunction]
pub(crate) fn effective_config_target_name<'py>(
    py: Python<'py>,
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
    selected: &Bound<'py, PyAny>,
) -> PyResult<Outcome<'py, Bound<'py, PyString>>> {
    let mut defined: Vec<Key<'py>> = keys(&native_field(project_config, "targets")?)?;
    defined.extend(keys(&native_field(local_config, "targets")?)?);
    let choice: TargetChoice<Key<'py>> = TargetChoice {
        selected: optional_key(selected)?,
        local: optional_key(&native_field(local_config, "target")?)?,
        default: optional_key(&native_field(project_config, "default_target")?)?,
        defined,
    };
    Ok(match effective_target_name(choice) {
        Ok(name) => (name.map(|name| name.handle), None),
        Err(error) => (None, Some(message(py, &error)?)),
    })
}

/// `resolve_target_config`: each merged `TargetConfig` field, or the ambiguous-connection message.
#[pyfunction]
pub(crate) fn effective_config_target_fields<'py>(
    py: Python<'py>,
    configs: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    target_name: &Bound<'py, PyAny>,
    default_target: &Bound<'py, PyAny>,
) -> PyResult<Outcome<'py, Vec<FieldRow<'py>>>> {
    let (project_config, local_config) = configs;
    let name: Key<'py> = key(target_name)?;
    let project_target: Option<Bound<'py, PyAny>> = native_field(&project_config, "targets")?
        .cast::<PyDict>()?
        .get_item(&name.handle)?;
    let local_target: Option<Bound<'py, PyAny>> = native_field(&local_config, "targets")?
        .cast::<PyDict>()?
        .get_item(&name.handle)?;
    let local: Option<Layer<'py>> = local_target.as_ref().map(target_layer).transpose()?;
    let mut merged: Layer<'py> = merged_target(
        target_layer(project_target.as_ref().unwrap_or(default_target))?,
        local.as_ref(),
    );
    let connection_name: usize = field_position("connection_name");
    let project_connections: Bound<'py, PyAny> = native_field(&project_config, "connections")?;
    let local_connections: Bound<'py, PyAny> = native_field(&local_config, "connections")?;
    let choice: ConnectionChoice<Key<'py>> = ConnectionChoice {
        target: name,
        selected: is_set(&merged.fields[connection_name])
            || is_set(&merged.fields[field_position("connection")])
            || native_field(&project_config, "connection")?.is_truthy()?
            || native_field(&local_config, "connection")?.is_truthy()?,
        project_names: kindless_keys(&project_connections, &local_connections)?,
        local_names: kindless_keys(&local_connections, &project_connections)?,
    };
    match default_connection_name(choice) {
        Ok(Some(only)) => {
            merged.fields[connection_name] = FieldValue::Scalar(Some(only.handle.into_any()));
        }
        Ok(None) => {}
        Err(error) => return Ok((None, Some(message(py, &error)?))),
    }
    let mut rows: Vec<FieldRow<'py>> = Vec::new();
    for (field, value) in target_fields().iter().zip(merged.fields) {
        let value: Value<'py> = match value {
            FieldValue::Scalar(value) => value.unwrap_or_else(|| py.None().into_bound(py)),
            FieldValue::Map(None) => py.None().into_bound(py),
            FieldValue::Map(Some(entries)) => dictionary(py, entries)?.into_any(),
        };
        rows.push((field.group, field.name, value));
    }
    Ok((Some(rows), None))
}

pub(crate) fn named<'py>(mapping: &Bound<'py, PyAny>) -> PyResult<Entries<Key<'py>, Pairs<'py>>> {
    mapping
        .cast::<PyDict>()?
        .iter()
        .map(|(name, connection)| Ok((key(&name)?, pairs(&connection)?)))
        .collect()
}

/// `build_effective_connection` before template expansion, or the unknown-connection message.
#[pyfunction]
pub(crate) fn effective_config_connection<'py>(
    py: Python<'py>,
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
    target_config: &Bound<'py, PyAny>,
) -> PyResult<Outcome<'py, Bound<'py, PyDict>>> {
    effective_connection_fields(py, (project_config, local_config), target_config, false)
}
