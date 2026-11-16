//! `${...}` templates in authored Python config values and effective vars, expanded natively.

use crate::model_config::models::PythonHost;
use crate::model_config::models::Stop;
use crate::model_config::models::TemplateSources;
use std::cell::RefCell;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyResult, Python};
use pyo3::types::{
    PyDict, PyDictMethods, PyList, PyListMethods, PyString, PyStringMethods, PyTuple,
    PyTupleMethods,
};
use sqlbuild_model_config::errors::ConfigError;
use sqlbuild_model_config::templates::errors::TemplateError;
use sqlbuild_model_config::templates::main::expand_template_string::expand_template_string;
use sqlbuild_model_config::templates::main::template_error_message::template_error_message;
use sqlbuild_model_config::templates::models::{StringExpansion, TemplateOptions};
use sqlbuild_model_config::templates::types::TemplateHost;

use crate::model_config::_helpers::config::config_errors::native_config_error;
use crate::model_config::errors::NativeConfigError;

const TEMPLATE_OPEN_TOKEN: &str = "${";
pub(crate) const ENVIRONMENT_READ: &str = "env";
pub(crate) const CONTEXT_READ: &str = "ctx";
pub(crate) const EFFECTIVE_VARS_LABEL: &str = "effective vars";
const VALUE_PATH: &str = "its value";
const BUILTINS_MODULE: &str = "builtins";
const ASCII_FUNCTION: &str = "ascii";
const ENCODE: &str = "encode";
const UTF32_LE: &str = "utf-32-le";
const SURROGATE_PASS: &str = "surrogatepass";

/// Effective vars resolved on demand, as Python's `_EffectiveVarsResolver` resolves them.
#[derive(Debug)]
pub(crate) struct EffectiveVars<'py> {
    pub(crate) resolved: Bound<'py, PyDict>,
    pub(crate) resolving: RefCell<Vec<String>>,
}

/// The `CompileInputError` Python raises for a template error in the context `label`.
pub(crate) fn template_error(error: &TemplateError, label: &str) -> ConfigError {
    ConfigError::compile(template_error_message(error, label))
}

/// The expanded value, or the native error expansion stopped at, and the reads in order.
pub(crate) type TemplateOutcome<'py> = (
    Result<Bound<'py, PyAny>, Py<NativeConfigError>>,
    Vec<(&'static str, String)>,
);

/// `value` with every template expanded under `options`, or the error, with its reads.
pub(crate) fn expand_config_templates<'py>(
    py: Python<'py>,
    value: &Bound<'py, PyAny>,
    sources: TemplateSources<'py>,
    (options, label): (TemplateOptions, &str),
) -> PyResult<TemplateOutcome<'py>> {
    let host = PythonHost::new(py, sources, label);
    let result = expanded(&host, value, options);
    outcome(py, host, result, label)
}

/// The first `(var name, value path, UTF-8 byte offset)` of a lone surrogate in raw vars.
pub(crate) fn lone_surrogate_site<'py>(
    raw_values: &Bound<'py, PyDict>,
) -> PyResult<Option<(Bound<'py, PyString>, String, usize)>> {
    for (key, value) in raw_values.iter() {
        if let Some((path, offset)) = surrogate_path(&value, VALUE_PATH.to_owned())? {
            return Ok(Some((key.str()?, path, offset)));
        }
    }
    Ok(None)
}

/// Python's former `_reject_lone_surrogates` walk: dict keys then items, lists and tuples.
fn surrogate_path(value: &Bound<'_, PyAny>, path: String) -> PyResult<Option<(String, usize)>> {
    if let Ok(mapping) = value.downcast::<PyDict>() {
        for (key, item) in mapping.iter() {
            if let Some(site) = surrogate_path(&key, format!("{path} key"))? {
                return Ok(Some(site));
            }
            let key_ascii = PyModule::import(value.py(), BUILTINS_MODULE)?
                .getattr(ASCII_FUNCTION)?
                .call1((&key,))?
                .str()?
                .to_string_lossy()
                .into_owned();
            if let Some(site) = surrogate_path(&item, format!("{path}[{key_ascii}]"))? {
                return Ok(Some(site));
            }
        }
        return Ok(None);
    }
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        for (index, item) in value.try_iter()?.enumerate() {
            if let Some(site) = surrogate_path(&item?, format!("{path}[{index}]"))? {
                return Ok(Some(site));
            }
        }
        return Ok(None);
    }
    let Ok(text) = value.downcast::<PyString>() else {
        return Ok(None);
    };
    Ok(first_surrogate_offset(text)?.map(|offset| (path, offset)))
}

/// The UTF-8 byte offset of the first lone surrogate in `text`.
fn first_surrogate_offset(text: &Bound<'_, PyString>) -> PyResult<Option<usize>> {
    let encoded = text.call_method1(ENCODE, (UTF32_LE, SURROGATE_PASS))?;
    let mut offset = 0;
    for unit in encoded.extract::<Vec<u8>>()?.as_chunks::<4>().0 {
        let code_point = u32::from_le_bytes([unit[0], unit[1], unit[2], unit[3]]);
        match char::from_u32(code_point) {
            Some(character) => offset += character.len_utf8(),
            None => return Ok(Some(offset)),
        }
    }
    Ok(None)
}

/// The resolved effective vars, or the native error resolution stopped at, with the reads.
pub(crate) fn resolve_effective_vars<'py>(
    py: Python<'py>,
    raw_values: &Bound<'py, PyDict>,
    environment: Bound<'py, PyAny>,
) -> PyResult<TemplateOutcome<'py>> {
    let mut host = PythonHost::new(
        py,
        TemplateSources(raw_values.clone(), environment, PyDict::new(py)),
        EFFECTIVE_VARS_LABEL,
    );
    let resolved = PyDict::new(py);
    host.effective = Some(EffectiveVars {
        resolved: resolved.clone(),
        resolving: RefCell::new(Vec::new()),
    });
    let mut result: Result<Bound<'py, PyAny>, Stop> = Ok(resolved.clone().into_any());
    for key in raw_values.keys() {
        let name = key.str()?.to_str()?.to_owned();
        if let Err(failure) = host.variable(&name) {
            result = Err(host.stop(failure));
            break;
        }
    }
    outcome(py, host, result, EFFECTIVE_VARS_LABEL)
}

fn outcome<'py>(
    py: Python<'py>,
    host: PythonHost<'py>,
    result: Result<Bound<'py, PyAny>, Stop>,
    label: &str,
) -> PyResult<TemplateOutcome<'py>> {
    let value = match result {
        Ok(value) => Ok(value),
        Err(Stop::Failure(error)) => Err(native_config_error(py, template_error(&error, label))?),
        Err(Stop::Python(error)) => return Err(error),
    };
    Ok((value, host.into_reads()))
}

/// Expand every template in `value` and the containers inside it.
pub(crate) fn expanded<'py>(
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
