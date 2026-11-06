//! Built model config trees retained natively, with Python-authored values converted on first read.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyclass;
use pyo3::types::{PyString, PyTuple};
use sqlbuild_model_config::config_values::errors::ConfigTypeError;
use sqlbuild_model_config::config_values::main::checked_cursor_bound::checked_cursor_bound;
use sqlbuild_model_config::config_values::main::checked_flag::checked_flag;
use sqlbuild_model_config::config_values::main::checked_integer::checked_integer;
use sqlbuild_model_config::config_values::main::checked_text::checked_text;
use sqlbuild_model_config::config_values::main::checked_texts::checked_texts;
use sqlbuild_model_config::config_values::models::{ConfigNode, CursorBound, HostChildren};

use crate::bindings::_helpers::compile_driver::stage_work::{TYPED_CONFIG_READS, count_stage};
use crate::bindings::_helpers::model_config::config_nodes::{
    HostHandle, PyConfigEntries, PyConfigNode, config_node, config_object, is_exact_container,
};
use crate::bindings::_helpers::model_config::config_values::config_value_type_error;
use crate::bindings::_helpers::records::fields::{
    cached_value, field as native_field, mapping_get, store_cached,
};
use crate::bindings::_helpers::records::models::NativeRecord;

const CONFIG_FIELD: &str = "config";
const VALUES_FIELD: &str = "values";
const ISOFORMAT_METHOD: &str = "isoformat";
/// The config record's cache slot holding its typed values; no Python attribute reads it.
const TYPED_VALUES_SLOT: &str = "__typed_values__";
const POISONED: &str = "typed config values cache is poisoned";

/// A built config's string-keyed tree; values shared across reused configs stay native.
pub(crate) type BuiltConfigValues = Arc<HashMap<String, Arc<PyConfigNode>>>;

/// The built tree and any Python-authored values opened by the record's readers.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct TypedConfigValues {
    pub(crate) built: Option<BuiltConfigValues>,
    read: Mutex<Vec<(String, Arc<PyConfigNode>)>>,
}

/// Keep the builder's tree without interpreting custom Python mapping keys natively.
pub(crate) fn built_values(entries: PyConfigEntries) -> Option<BuiltConfigValues> {
    let mut values = HashMap::with_capacity(entries.len());
    for (key, value) in entries {
        let ConfigNode::Str(key) = key else {
            return None;
        };
        values.insert(key, Arc::new(value));
    }
    Some(Arc::new(values))
}

/// Attach the built tree to its public record, rather than reconstructing it from that view.
pub(crate) fn retain_values(
    config: &Bound<'_, PyAny>,
    built: Option<&BuiltConfigValues>,
) -> PyResult<()> {
    if let Some(built) = built
        && let Ok(record) = config.cast::<NativeRecord>()
    {
        let typed = Bound::new(
            config.py(),
            TypedConfigValues {
                built: Some(Arc::clone(built)),
                read: Mutex::new(Vec::new()),
            },
        )?;
        store_cached(record, TYPED_VALUES_SLOT, &typed.into_any().unbind())?;
    }
    Ok(())
}

/// A config record's values and the typed nodes read of them, each key converted on first read.
pub(crate) type ConfigValues<'py> = (Bound<'py, PyAny>, Bound<'py, TypedConfigValues>);

/// `values.get(key)` as a typed node, `None` when absent; each key is converted once.
fn node(values: &ConfigValues<'_>, key: &str) -> PyResult<Arc<PyConfigNode>> {
    let (mapping, typed) = values;
    let py: Python<'_> = mapping.py();
    let built = typed.get().built.as_ref().map(|values| values.get(key));
    let deferred = match built {
        Some(Some(node)) => match node.as_ref() {
            ConfigNode::Host(host)
                if matches!(host.children, HostChildren::None)
                    && is_exact_container(host.handle.0.bind(py)) =>
            {
                Some(host.handle.0.bind(py))
            }
            _ => return Ok(Arc::clone(node)),
        },
        Some(None) => return Ok(Arc::new(ConfigNode::Null)),
        None => None,
    };
    let read: &Mutex<Vec<(String, Arc<PyConfigNode>)>> = &typed.get().read;
    let poisoned = || PyRuntimeError::new_err(POISONED);
    let found: Option<Arc<PyConfigNode>> = read
        .lock()
        .map_err(|_| poisoned())?
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, node)| Arc::clone(node));
    if let Some(node) = found {
        return Ok(node);
    }
    let node: Arc<PyConfigNode> = Arc::new(match deferred {
        Some(value) => config_node(value)?,
        None => config_node(&mapping_get(mapping, key)?)?,
    });
    count_stage(py, TYPED_CONFIG_READS, 1)?;
    read.lock()
        .map_err(|_| poisoned())?
        .push((key.to_owned(), Arc::clone(&node)));
    Ok(node)
}

/// A config record's values, their typed nodes kept on the record so each key converts once.
pub(crate) fn typed_values<'py>(config: &Bound<'py, PyAny>) -> PyResult<ConfigValues<'py>> {
    let py: Python<'py> = config.py();
    let values: Bound<'py, PyAny> = native_field(config, VALUES_FIELD)?;
    let fresh = || {
        Bound::new(
            py,
            TypedConfigValues {
                built: None,
                read: Mutex::new(Vec::new()),
            },
        )
    };
    let Ok(record) = config.cast::<NativeRecord>() else {
        let typed: Bound<'py, TypedConfigValues> = fresh()?;
        return Ok((values, typed));
    };
    let typed: Bound<'py, TypedConfigValues> = match cached_value(record, TYPED_VALUES_SLOT)? {
        Some(typed) => typed.into_bound(py).cast_into()?,
        None => {
            let typed: Bound<'py, TypedConfigValues> = fresh()?;
            store_cached(
                record,
                TYPED_VALUES_SLOT,
                &typed.clone().into_any().unbind(),
            )?;
            typed
        }
    };
    Ok((values, typed))
}

/// A model's config values as typed nodes, read key by key.
pub(crate) fn model_values<'py>(model: &Bound<'py, PyAny>) -> PyResult<ConfigValues<'py>> {
    typed_values(&native_field(model, CONFIG_FIELD)?)
}

/// The `ConfigValueTypeError` a typed read raises, naming the value's Python type.
pub(crate) fn type_error(py: Python<'_>, error: &ConfigTypeError<'_, HostHandle>) -> PyErr {
    let raised = || -> PyResult<PyErr> {
        let actual_type = config_object(py, error.value)?.get_type().into_any();
        Ok(PyErr::from_value(config_value_type_error(
            &error.key,
            error.expected,
            &actual_type,
        )?))
    };
    raised().unwrap_or_else(|error| error)
}

/// The exact text of a string node; a string Python cannot encode raises as reading it does.
pub(crate) fn node_text(py: Python<'_>, node: &PyConfigNode) -> PyResult<String> {
    match node {
        ConfigNode::Str(text) => Ok(text.clone()),
        ConfigNode::Host(host) if host.facts.strict_text => {
            Ok(host.facts.text.clone().unwrap_or_default())
        }
        _ => config_object(py, node)?.extract(),
    }
}

/// `key`'s string value, or `None` when absent or `None`.
pub(crate) fn config_text(
    py: Python<'_>,
    values: &ConfigValues<'_>,
    key: &str,
) -> PyResult<Option<String>> {
    let node: Arc<PyConfigNode> = node(values, key)?;
    checked_text(key, Some(&node))
        .map_err(|error| type_error(py, &error))?
        .map(|value| node_text(py, value))
        .transpose()
}

/// `key`'s string value as its Python object, or `None` when absent or `None`.
pub(crate) fn config_text_object<'py>(
    py: Python<'py>,
    values: &ConfigValues<'_>,
    key: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let node: Arc<PyConfigNode> = node(values, key)?;
    checked_text(key, Some(&node))
        .map_err(|error| type_error(py, &error))?
        .map(|value| config_object(py, value))
        .transpose()
}

/// Whether `key` holds a string equal to `expected`, as Python's `==` compares them.
pub(crate) fn config_text_is(
    py: Python<'_>,
    values: &ConfigValues<'_>,
    key: &str,
    expected: &str,
) -> PyResult<bool> {
    let node: Arc<PyConfigNode> = node(values, key)?;
    let text: Option<&PyConfigNode> =
        checked_text(key, Some(&node)).map_err(|error| type_error(py, &error))?;
    Ok(match text {
        Some(ConfigNode::Str(text)) => text == expected,
        Some(ConfigNode::Host(host)) if host.facts.strict_text => {
            host.facts.text.as_deref() == Some(expected)
        }
        _ => false,
    })
}

/// A cursor bound's text object: a string's or integer's `str()`, a date's `isoformat()`.
pub(crate) fn cursor_bound_object<'py>(
    py: Python<'py>,
    bound: CursorBound<'_, HostHandle>,
) -> PyResult<Bound<'py, PyAny>> {
    match bound {
        CursorBound::Dated(value) => config_object(py, value)?.call_method0(ISOFORMAT_METHOD),
        CursorBound::Stringified(ConfigNode::Str(text)) => Ok(PyString::new(py, text).into_any()),
        CursorBound::Stringified(ConfigNode::Int(number)) => {
            Ok(PyString::new(py, &number.to_string()).into_any())
        }
        CursorBound::Stringified(value) => Ok(config_object(py, value)?.str()?.into_any()),
    }
}

/// `key`'s cursor bound as text, or `None` when absent or `None`.
pub(crate) fn config_cursor_bound<'py>(
    py: Python<'py>,
    values: &ConfigValues<'_>,
    key: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let node: Arc<PyConfigNode> = node(values, key)?;
    checked_cursor_bound(key, Some(&node))
        .map_err(|error| type_error(py, &error))?
        .map(|bound| cursor_bound_object(py, bound))
        .transpose()
}

/// `values.get(key)` as its Python object, `None` when absent.
pub(crate) fn config_value<'py>(
    py: Python<'py>,
    values: &ConfigValues<'_>,
    key: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let value: Arc<PyConfigNode> = node(values, key)?;
    config_object(py, &value)
}

/// `values.get(key)` as its typed node, `None` (null) when absent.
pub(crate) fn config_node_of(values: &ConfigValues<'_>, key: &str) -> PyResult<Arc<PyConfigNode>> {
    node(values, key)
}

/// `key`'s boolean value, or `None` when absent or `None`.
pub(crate) fn config_flag(
    py: Python<'_>,
    values: &ConfigValues<'_>,
    key: &str,
) -> PyResult<Option<bool>> {
    let node: Arc<PyConfigNode> = node(values, key)?;
    checked_flag(key, Some(&node)).map_err(|error| type_error(py, &error))
}

/// `key`'s integer value as its Python object, or `None` when absent or `None`.
pub(crate) fn config_integer_object<'py>(
    py: Python<'py>,
    values: &ConfigValues<'_>,
    key: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let node: Arc<PyConfigNode> = node(values, key)?;
    checked_integer(key, Some(&node))
        .map_err(|error| type_error(py, &error))?
        .map(|value| config_object(py, value))
        .transpose()
}

/// `key`'s strings as a tuple, or `None` when absent or `None`.
pub(crate) fn config_text_tuple<'py>(
    py: Python<'py>,
    values: &ConfigValues<'_>,
    key: &str,
) -> PyResult<Option<Bound<'py, PyTuple>>> {
    let node: Arc<PyConfigNode> = node(values, key)?;
    let Some(items) = checked_texts(key, Some(&node)).map_err(|error| type_error(py, &error))?
    else {
        return Ok(None);
    };
    let objects: Vec<Bound<'py, PyAny>> = items
        .into_iter()
        .map(|item| config_object(py, item))
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, objects).map(Some)
}
