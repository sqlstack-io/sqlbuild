//! Effective target, vars, connection and settings precedence over the loaded config objects.

use std::hash::{Hash, Hasher};

use crate::bindings::_helpers::records::fields::field as native_field;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, PyStringMethods, Python,
};
use pyo3::types::{PyBytes, PyBytesMethods, PyDict, PyDictMethods, PyListMethods, PyString};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_config::effective::errors::{EffectiveConfigError, MessagePart};
use sqlbuild_config::effective::main::default_connection_name::default_connection_name;
use sqlbuild_config::effective::main::effective_connection::effective_connection;
use sqlbuild_config::effective::main::effective_settings::effective_settings;
use sqlbuild_config::effective::main::effective_target_name::effective_target_name;
use sqlbuild_config::effective::main::effective_vars::effective_vars;
use sqlbuild_config::effective::main::merged_target::merged_target;
use sqlbuild_config::effective::main::target_fields::target_fields;
use sqlbuild_config::effective::models::{
    ConnectionChoice, ConnectionLayers, FieldValue, MergeRule, TargetChoice, TargetField,
    TargetLayer,
};
use sqlbuild_config::effective::types::Entries;

const UTF8: &str = "utf-8";
const SURROGATE_PASS: &str = "surrogatepass";

/// A `str` key compared exactly: by its UTF-8 bytes, lone surrogates passed through.
#[derive(Clone)]
struct Key<'py> {
    bytes: Vec<u8>,
    handle: Bound<'py, PyString>,
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

type Value<'py> = Bound<'py, PyAny>;
type Layer<'py> = TargetLayer<Key<'py>, Value<'py>>;
type Pairs<'py> = Entries<Key<'py>, Value<'py>>;
/// `(group, name, value)` of each merged `TargetConfig` field.
type FieldRow<'py> = (Option<&'static str>, &'static str, Value<'py>);
/// A result, else the precedence failure message.
type Outcome<'py, T> = (Option<T>, Option<Bound<'py, PyString>>);

fn key<'py>(value: &Bound<'py, PyAny>) -> PyResult<Key<'py>> {
    let handle: Bound<'py, PyString> = value.downcast::<PyString>()?.clone();
    if let Ok(text) = handle.to_str() {
        return Ok(Key {
            bytes: text.as_bytes().to_vec(),
            handle,
        });
    }
    let bytes: Vec<u8> = handle
        .call_method1("encode", (UTF8, SURROGATE_PASS))?
        .downcast_into::<PyBytes>()?
        .as_bytes()
        .to_vec();
    Ok(Key { bytes, handle })
}

fn optional_key<'py>(value: &Bound<'py, PyAny>) -> PyResult<Option<Key<'py>>> {
    if value.is_none() {
        return Ok(None);
    }
    key(value).map(Some)
}

fn pairs<'py>(mapping: &Bound<'py, PyAny>) -> PyResult<Pairs<'py>> {
    mapping
        .downcast::<PyDict>()?
        .iter()
        .map(|(name, value)| Ok((key(&name)?, value)))
        .collect()
}

fn keys<'py>(mapping: &Bound<'py, PyAny>) -> PyResult<Vec<Key<'py>>> {
    mapping
        .downcast::<PyDict>()?
        .keys()
        .iter()
        .map(|name| key(&name))
        .collect()
}

fn dictionary<'py>(py: Python<'py>, entries: Pairs<'py>) -> PyResult<Bound<'py, PyDict>> {
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
fn message<'py>(
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
        .downcast_into::<PyString>()?)
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
        .downcast::<PyDict>()?
        .get_item(&name.handle)?;
    let local_target: Option<Bound<'py, PyAny>> = native_field(&local_config, "targets")?
        .downcast::<PyDict>()?
        .get_item(&name.handle)?;
    let local: Option<Layer<'py>> = local_target.as_ref().map(target_layer).transpose()?;
    let mut merged: Layer<'py> = merged_target(
        target_layer(project_target.as_ref().unwrap_or(default_target))?,
        local.as_ref(),
    );
    let connection_name: usize = field_position("connection_name");
    let choice: ConnectionChoice<Key<'py>> = ConnectionChoice {
        target: name,
        selected: is_set(&merged.fields[connection_name])
            || is_set(&merged.fields[field_position("connection")])
            || native_field(&project_config, "connection")?.is_truthy()?
            || native_field(&local_config, "connection")?.is_truthy()?,
        project_names: keys(&native_field(&project_config, "connections")?)?,
        local_names: keys(&native_field(&local_config, "connections")?)?,
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

/// `build_effective_vars` before template expansion.
#[pyfunction]
pub(crate) fn effective_config_vars<'py>(
    py: Python<'py>,
    configs: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    target_config: &Bound<'py, PyAny>,
    cli_vars: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let (project_config, local_config) = configs;
    let project: Pairs<'py> = pairs(&native_field(&project_config, "vars")?)?;
    let target: Pairs<'py> = if target_config.is_none() {
        Vec::new()
    } else {
        pairs(&native_field(target_config, "vars")?)?
    };
    let local: Pairs<'py> = pairs(&native_field(&local_config, "vars")?)?;
    let cli: Pairs<'py> = pairs(cli_vars)?;
    dictionary(py, effective_vars(&[&project, &target, &local, &cli]))
}

fn named<'py>(mapping: &Bound<'py, PyAny>) -> PyResult<Entries<Key<'py>, Pairs<'py>>> {
    mapping
        .downcast::<PyDict>()?
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

/// Catalog selection uses the same named-layer resolver, with no writer layers.
pub(crate) fn effective_connection_fields<'py>(
    py: Python<'py>,
    (project_config, local_config): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    target_config: &Bound<'py, PyAny>,
    catalog: bool,
) -> PyResult<Outcome<'py, Bound<'py, PyDict>>> {
    let catalog_name = if catalog && !target_config.is_none() {
        optional_key(&native_field(target_config, "catalog_connection")?)?
    } else {
        None
    };
    let independent = catalog_name.is_some();
    let target: Option<(Option<Key<'py>>, Pairs<'py>)> = if independent {
        Some((catalog_name, Vec::new()))
    } else if target_config.is_none() {
        None
    } else {
        Some((
            optional_key(&native_field(target_config, "connection_name")?)?,
            pairs(&native_field(target_config, "connection")?)?,
        ))
    };
    let layers: ConnectionLayers<Key<'py>, Value<'py>> = ConnectionLayers {
        project: if independent {
            Vec::new()
        } else {
            pairs(&native_field(project_config, "connection")?)?
        },
        target,
        project_named: named(&native_field(project_config, "connections")?)?,
        local_named: named(&native_field(local_config, "connections")?)?,
        local: if independent {
            Vec::new()
        } else {
            pairs(&native_field(local_config, "connection")?)?
        },
    };
    Ok(match effective_connection(layers) {
        Ok(connection) => (Some(dictionary(py, connection)?), None),
        Err(error) => {
            let message = message(py, &error)?;
            (
                None,
                Some(if independent {
                    PyString::new(py, &format!("catalog_connection: {message}"))
                } else {
                    message
                }),
            )
        }
    })
}

/// `build_effective_settings`: setting values by field name, local overrides applied.
#[pyfunction]
pub(crate) fn effective_config_settings<'py>(
    py: Python<'py>,
    settings: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    overrides: &Bound<'py, PyAny>,
    names: Vec<Bound<'py, PyString>>,
) -> PyResult<Bound<'py, PyDict>> {
    let read = |settings: &Bound<'py, PyAny>| -> PyResult<Pairs<'py>> {
        names
            .iter()
            .map(|name| {
                Ok((
                    key(name.as_any())?,
                    native_field(settings, &name.to_cow()?)?,
                ))
            })
            .collect()
    };
    let overridden: Vec<Key<'py>> = overrides
        .try_iter()?
        .map(|name| key(&name?))
        .collect::<PyResult<_>>()?;
    let (project_settings, local_settings) = settings;
    let settings: Pairs<'py> = effective_settings(
        read(&project_settings)?,
        &read(&local_settings)?,
        &overridden,
    );
    dictionary(py, settings)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(effective_config_target_name, module)?)?;
    module.add_function(wrap_pyfunction!(effective_config_target_fields, module)?)?;
    module.add_function(wrap_pyfunction!(effective_config_vars, module)?)?;
    module.add_function(wrap_pyfunction!(effective_config_connection, module)?)?;
    module.add_function(wrap_pyfunction!(effective_config_settings, module)?)?;
    Ok(())
}
