//! Expand every template in `value` and the containers inside it.

use crate::model_config::_helpers::config::config_templates::TEMPLATE_OPEN_TOKEN;
use crate::model_config::models::PythonHost;
use crate::model_config::models::Stop;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods};
use pyo3::types::{
    PyDict, PyDictMethods, PyList, PyListMethods, PyString, PyStringMethods, PyTuple,
    PyTupleMethods,
};
use sqlbuild_model_config::templates::main::expand_template_string::expand_template_string;
use sqlbuild_model_config::templates::models::{StringExpansion, TemplateOptions};

/// Expand every template in `value` and the containers inside it.
pub fn expanded<'py>(
    host: &PythonHost<'py>,
    value: &Bound<'py, PyAny>,
    options: TemplateOptions,
) -> Result<Bound<'py, PyAny>, Stop> {
    let py = host.py;
    if let Ok(text) = value.downcast::<PyString>() {
        let text = text.to_str().map_err(Stop::Python)?;
        if !text.contains(TEMPLATE_OPEN_TOKEN) {
            return Ok(value.clone());
        }
        return match expand_template_string(host, text, options) {
            Ok(StringExpansion::Unchanged) => Ok(PyString::new(py, text).into_any()),
            Ok(StringExpansion::Value(result)) => Ok(result),
            Ok(StringExpansion::Text(result)) => Ok(PyString::new(py, &result).into_any()),
            Err(failure) => Err(host.stop(failure)),
        };
    }
    if let Ok(mapping) = value.downcast::<PyDict>() {
        let result = PyDict::new(py);
        for (key, item) in mapping.iter() {
            result
                .set_item(key, expanded(host, &item, options)?)
                .map_err(Stop::Python)?;
        }
        return Ok(result.into_any());
    }
    if let Ok(items) = value.downcast::<PyList>() {
        let result = items
            .iter()
            .map(|item| expanded(host, &item, options))
            .collect::<Result<Vec<_>, _>>()?;
        return PyList::new(py, result)
            .map(Bound::into_any)
            .map_err(Stop::Python);
    }
    if let Ok(items) = value.downcast::<PyTuple>() {
        let result = items
            .iter()
            .map(|item| expanded(host, &item, options))
            .collect::<Result<Vec<_>, _>>()?;
        return PyTuple::new(py, result)
            .map(Bound::into_any)
            .map_err(Stop::Python);
    }
    Ok(value.clone())
}
