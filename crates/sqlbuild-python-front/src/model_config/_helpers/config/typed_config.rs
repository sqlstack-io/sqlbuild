//! Built model config trees retained natively, with Python-authored values converted on first read.

use crate::model_config::main::config::config_node::config_node;
use crate::model_config::main::config::config_object::config_object;
use crate::model_config::types::BuiltConfigValues;
use crate::model_config::types::ConfigValues;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_model_config::config_values::errors::ConfigTypeError;
use sqlbuild_model_config::config_values::models::{ConfigNode, CursorBound, HostChildren};

use crate::model_config::_helpers::config::config_nodes::{PyConfigEntries, is_exact_container};
use crate::model_config::_helpers::config::config_values::config_value_type_error;
use crate::model_config::models::HostHandle;
use crate::model_config::types::PyConfigNode;
use sqlbuild_python_base::compile_driver::constants::TYPED_CONFIG_READS;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;

pub(crate) const CONFIG_FIELD: &str = "config";
pub(crate) const VALUES_FIELD: &str = "values";
const ISOFORMAT_METHOD: &str = "isoformat";
/// The config record's cache slot holding its typed values; no Python attribute reads it.
pub(crate) const TYPED_VALUES_SLOT: &str = "__typed_values__";
const POISONED: &str = "typed config values cache is poisoned";

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

/// `values.get(key)` as a typed node, `None` when absent; each key is converted once.
pub(crate) fn node(values: &ConfigValues<'_>, key: &str) -> PyResult<Arc<PyConfigNode>> {
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
