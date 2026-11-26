//! Secret-safe diagnostic text and structured logging field values.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{
    PyBool, PyDict, PyDictMethods, PyFloat, PyInt, PyList, PyString, PyStringMethods, PyTuple,
    PyTypeMethods,
};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::redaction::main::redact_text::redact_text;
use sqlbuild_core::redaction::main::secret_field_name::secret_field_name;

use crate::boundary::main::imported::imported;
use crate::records::main::fields::mapping_items::mapping_items;

const REDACTED: &str = "[REDACTED]";

/// Obvious named secrets and URI passwords redacted from diagnostic text.
#[pyfunction]
#[pyo3(signature = (value))]
fn redact_diagnostic_text(value: &str) -> String {
    redact_text(value)
}

fn redacted_mapping<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    let redacted: Bound<'py, PyDict> = PyDict::new(py);
    for item in mapping_items(value)?.try_iter()? {
        let (key, item): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        let name: String = key.str()?.extract()?;
        redacted.set_item(&name, redacted_value(&name, &item)?)?;
    }
    Ok(redacted.into_any())
}

/// A persistence-safe value for one structured logging field.
pub(crate) fn redacted_value<'py>(
    name: &str,
    value: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    if secret_field_name(name) {
        return Ok(PyString::new(py, REDACTED).into_any());
    }
    if let Ok(text) = value.cast::<PyString>() {
        return Ok(PyString::new(py, &redact_text(text.to_str()?)).into_any());
    }
    let scalar: bool = value.is_none()
        || value.is_instance_of::<PyBool>()
        || value.is_instance_of::<PyInt>()
        || value.is_instance_of::<PyFloat>();
    if scalar {
        return Ok(value.clone());
    }
    if value.is_instance(&imported(py, "collections.abc", "Mapping")?)? {
        return redacted_mapping(value);
    }
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        let items: Vec<Bound<'py, PyAny>> = value
            .try_iter()?
            .map(|item| redacted_value(name, &item?))
            .collect::<PyResult<_>>()?;
        return Ok(PyList::new(py, items)?.into_any());
    }
    let rendered: PyResult<String> = value.str().and_then(|text| text.extract());
    Ok(match rendered {
        Ok(text) => PyString::new(py, &redact_text(&text)).into_any(),
        Err(_) => {
            let type_name: String = value.get_type().name()?.extract()?;
            PyString::new(py, &format!("<{type_name}>")).into_any()
        }
    })
}

/// A persistence-safe value for one structured logging field.
#[pyfunction]
#[pyo3(signature = (*, name, value))]
fn redact_diagnostic_value<'py>(
    name: &str,
    value: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    redacted_value(name, value)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(redact_diagnostic_text, module)?)?;
    module.add_function(wrap_pyfunction!(redact_diagnostic_value, module)?)?;
    Ok(())
}
