//! The Python object of one config value: new builtins, and opaque values as themselves.

use crate::model_config::_helpers::config::config_nodes::config_dict;
use crate::model_config::types::PyConfigNode;
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::{PyBool, PyFloat, PyList, PyString, PyTuple};
use sqlbuild_model_config::config_values::models::ConfigNode;

/// The Python object of one config value: new builtins, and opaque values as themselves.
pub fn config_object<'py>(py: Python<'py>, node: &PyConfigNode) -> PyResult<Bound<'py, PyAny>> {
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
