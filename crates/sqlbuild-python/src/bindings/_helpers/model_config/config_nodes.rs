//! Python config values converted once into `ConfigNode`s, and projected back into Python objects.

use std::sync::Arc;

use pyo3::IntoPyObject;
use pyo3::exceptions::{PyOverflowError, PyUnicodeEncodeError};
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{
    PyBool, PyDict, PyDictMethods, PyFloat, PyInt, PyList, PyListMethods, PyString,
    PyStringMethods, PyTuple, PyTupleMethods,
};
use sqlbuild_model_config::config_values::models::{
    ConfigNode, HostChildren, HostFacts, HostValue,
};
use sqlbuild_model_config::config_values::types::HostObject;
use sqlbuild_model_config::types::{AuthoredNode, NodeKind};

use crate::bindings::_helpers::model_config::authored_nodes::PyNode;
use crate::bindings::_helpers::records::construct::record_class;
use crate::bindings::_helpers::records::ids::RecordId;

/// The typed hook entry classes, which the config build checks hook lists against.
const HOOK_ENTRY_CLASSES: [RecordId; 3] = [
    RecordId::SqlHookEntry,
    RecordId::NamedSqlHookEntry,
    RecordId::PythonHookEntry,
];

/// The Python object an opaque config value stands for.
#[derive(Clone, Debug)]
pub(crate) struct HostHandle(pub(crate) Arc<Py<PyAny>>);

impl HostObject for HostHandle {
    fn is_date_like(&self) -> bool {
        Python::attach(|py| PyNode(self.0.bind(py).clone()).is_date_like())
    }

    /// Python's text of the projected value; a `str()` or `repr()` that raises reads as empty, as `PyNode`.
    fn display(node: &ConfigNode<Self>, repr: bool) -> String {
        Python::attach(|py| {
            config_object(py, node).map_or_else(
                |_| String::new(),
                |value| {
                    let value: PyNode<'_> = PyNode(value);
                    if repr {
                        value.python_repr()
                    } else {
                        value.python_str()
                    }
                },
            )
        })
    }
}
/// A config value converted from Python.
pub(crate) type PyConfigNode = ConfigNode<HostHandle>;
/// A converted config mapping's entries.
pub(crate) type PyConfigEntries = Vec<(PyConfigNode, PyConfigNode)>;

/// Whether a Python `str` encodes to UTF-8, which it does unless it holds a lone surrogate.
fn utf8_text(text: &Bound<'_, PyString>) -> PyResult<bool> {
    match text.to_str() {
        Ok(_) => Ok(true),
        Err(error) if error.is_instance_of::<PyUnicodeEncodeError>(text.py()) => Ok(false),
        Err(error) => Err(error),
    }
}

/// An opaque value with the cheap facts `PyNode` reads of it, and its converted items.
fn host(value: &Bound<'_, PyAny>, children: HostChildren<HostHandle>) -> PyResult<PyConfigNode> {
    let node: PyNode<'_> = PyNode(value.clone());
    let strict_text: bool = match value.downcast::<PyString>() {
        Ok(text) => utf8_text(text)?,
        Err(_) => false,
    };
    Ok(ConfigNode::Host(Box::new(HostValue {
        handle: HostHandle(Arc::new(value.clone().unbind())),
        facts: HostFacts {
            kind: node.kind(),
            text: node.text(),
            strict_text,
            integer: node.integer(),
            number: node.number(),
        },
        children,
    })))
}

/// Converted items, or `None` when a container holds itself; `open` holds the enclosing containers.
fn converted_items<'py>(
    items: impl Iterator<Item = Bound<'py, PyAny>>,
    open: &[usize],
) -> PyResult<Option<Vec<PyConfigNode>>> {
    let mut converted: Vec<PyConfigNode> = Vec::new();
    for item in items {
        match convert(&item, open)? {
            Some(node) => converted.push(node),
            None => return Ok(None),
        }
    }
    Ok(Some(converted))
}

fn converted_entries(
    mapping: &Bound<'_, PyDict>,
    open: &[usize],
) -> PyResult<Option<PyConfigEntries>> {
    let mut converted: PyConfigEntries = Vec::with_capacity(mapping.len());
    for (key, item) in mapping.iter() {
        let (Some(key), Some(item)) = (convert(&key, open)?, convert(&item, open)?) else {
            return Ok(None);
        };
        converted.push((key, item));
    }
    Ok(Some(converted))
}

/// A container's converted children, or `None` when it holds itself.
fn container(
    value: &Bound<'_, PyAny>,
    enclosing: &[usize],
) -> PyResult<Option<HostChildren<HostHandle>>> {
    let address: usize = value.as_ptr() as usize;
    if enclosing.contains(&address) {
        return Ok(None);
    }
    let open: Vec<usize> = [enclosing, &[address]].concat();
    Ok(if let Ok(mapping) = value.downcast::<PyDict>() {
        converted_entries(mapping, &open)?.map(HostChildren::Entries)
    } else if let Ok(items) = value.downcast::<PyList>() {
        converted_items(items.iter(), &open)?.map(HostChildren::Items)
    } else if let Ok(items) = value.downcast::<PyTuple>() {
        converted_items(items.iter(), &open)?.map(HostChildren::Items)
    } else {
        Some(HostChildren::None)
    })
}

fn convert(value: &Bound<'_, PyAny>, open: &[usize]) -> PyResult<Option<PyConfigNode>> {
    if value.is_none() {
        return Ok(Some(ConfigNode::Null));
    }
    if value.is_exact_instance_of::<PyBool>() {
        return Ok(Some(ConfigNode::Bool(value.is_truthy()?)));
    }
    if value.is_exact_instance_of::<PyInt>() {
        match value.extract::<i64>() {
            Ok(number) => return Ok(Some(ConfigNode::Int(number))),
            Err(error) if error.is_instance_of::<PyOverflowError>(value.py()) => {}
            Err(error) => return Err(error),
        }
    }
    if value.is_exact_instance_of::<PyFloat>() {
        return Ok(Some(ConfigNode::Float(value.extract::<f64>()?)));
    }
    if let Ok(text) = value.downcast_exact::<PyString>()
        && utf8_text(text)?
    {
        return Ok(Some(ConfigNode::Str(text.to_str()?.to_owned())));
    }
    let Some(children) = container(value, open)? else {
        return Ok(None);
    };
    let exact: bool = value.is_exact_instance_of::<PyDict>()
        || value.is_exact_instance_of::<PyList>()
        || value.is_exact_instance_of::<PyTuple>();
    Ok(Some(match children {
        HostChildren::Entries(entries) if exact => ConfigNode::Map(entries),
        HostChildren::Items(items) if exact && value.is_exact_instance_of::<PyList>() => {
            ConfigNode::List(items)
        }
        HostChildren::Items(items) if exact => ConfigNode::Tuple(items),
        children => host(value, children)?,
    }))
}

/// Carry an uninspected value: exact containers keep their own objects without opening items.
pub(crate) fn carried_node(value: &Bound<'_, PyAny>) -> PyResult<PyConfigNode> {
    if is_exact_container(value) {
        host(value, HostChildren::None)
    } else {
        config_node(value)
    }
}

/// Whether `value` is an exact `list`, `tuple` or `dict`.
pub(crate) fn is_exact_container(value: &Bound<'_, PyAny>) -> bool {
    value.is_exact_instance_of::<PyList>()
        || value.is_exact_instance_of::<PyTuple>()
        || value.is_exact_instance_of::<PyDict>()
}

/// One Python config value as a `ConfigNode`; a value that holds itself stays opaque whole.
pub(crate) fn config_node(value: &Bound<'_, PyAny>) -> PyResult<PyConfigNode> {
    match convert(value, &[])? {
        Some(node) => Ok(node),
        None => host(value, HostChildren::None),
    }
}

/// A Python mapping's entries as config entries.
pub(crate) fn config_entries(mapping: &Bound<'_, PyDict>) -> PyResult<PyConfigEntries> {
    mapping
        .iter()
        .map(|(key, value)| Ok((config_node(&key)?, config_node(&value)?)))
        .collect()
}

/// The Python object of one config value: new builtins, and opaque values as themselves.
pub(crate) fn config_object<'py>(
    py: Python<'py>,
    node: &PyConfigNode,
) -> PyResult<Bound<'py, PyAny>> {
    Ok(match node {
        ConfigNode::Null => py.None().into_bound(py),
        ConfigNode::Bool(flag) => PyBool::new(py, *flag).to_owned().into_any(),
        ConfigNode::Int(number) => number.into_pyobject(py)?.into_any(),
        ConfigNode::Float(number) => PyFloat::new(py, *number).into_any(),
        ConfigNode::Str(text) => PyString::new(py, text).into_any(),
        ConfigNode::List(items) => PyList::new(
            py,
            items
                .iter()
                .map(|item| config_object(py, item))
                .collect::<PyResult<Vec<_>>>()?,
        )?
        .into_any(),
        ConfigNode::Tuple(items) => PyTuple::new(
            py,
            items
                .iter()
                .map(|item| config_object(py, item))
                .collect::<PyResult<Vec<_>>>()?,
        )?
        .into_any(),
        ConfigNode::Map(entries) => config_dict(py, entries)?.into_any(),
        ConfigNode::Host(host) => host.handle.0.bind(py).clone(),
    })
}

/// A new Python `dict` of config entries.
pub(crate) fn config_dict<'py>(
    py: Python<'py>,
    entries: &[(PyConfigNode, PyConfigNode)],
) -> PyResult<Bound<'py, PyDict>> {
    let mapping: Bound<'py, PyDict> = PyDict::new(py);
    for (key, value) in entries {
        mapping.set_item(config_object(py, key)?, config_object(py, value)?)?;
    }
    Ok(mapping)
}

/// Whether `node` kind is a Python `str` instance, as `isinstance(value, str)` reads it.
pub(crate) fn is_text_node(node: &PyConfigNode) -> bool {
    node.kind() == NodeKind::Str
}

pub(crate) fn hook_entry(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    for class in HOOK_ENTRY_CLASSES {
        if value.is_instance(&record_class(value.py(), class)?)? {
            return Ok(true);
        }
    }
    Ok(false)
}
