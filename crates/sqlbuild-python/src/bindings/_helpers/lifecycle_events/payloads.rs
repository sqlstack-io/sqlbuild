//! Frozen JSON values read from Python into payload values, and JSON values frozen in place.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyFloat, PyInt, PyList, PyString, PyTuple};
use sqlbuild_core::lifecycle_events::models::PayloadValue;
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_python_base::native_host::main::host_error::host_error;

use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;

/// `ObservabilityValidationError(message)`.
pub(crate) fn validation_error(py: Python<'_>, message: &str) -> PyErr {
    host_error(py, ExecutorHost::ObservabilityValidationError, message)
}

/// Whether `value` is a `collections.abc.Mapping`.
pub(crate) fn abc_mapping(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    value.is_instance(&imported(value.py(), "collections.abc", "Mapping")?)
}

/// One segment of a JSON value path: a key or a list index.
#[derive(Clone)]
pub(crate) enum PathSegment {
    Key(String),
    Index(usize),
}

/// The root name, then `.key` and `[index]` segments.
fn rendered_path(path: &[PathSegment]) -> String {
    let mut text: String = String::new();
    for (position, segment) in path.iter().enumerate() {
        match segment {
            PathSegment::Index(index) => text.push_str(&format!("[{index}]")),
            PathSegment::Key(key) if position > 0 => text.push_str(&format!(".{key}")),
            PathSegment::Key(key) => text.push_str(key),
        }
    }
    text
}

/// `path` followed by one more segment.
fn extended(path: &[PathSegment], segment: PathSegment) -> Vec<PathSegment> {
    let mut extended: Vec<PathSegment> = path.to_vec();
    extended.push(segment);
    extended
}

fn scalar(value: &Bound<'_, PyAny>) -> bool {
    value.is_none() || value.is_instance_of::<PyString>() || value.is_instance_of::<PyInt>()
}

/// Validate and recursively freeze a JSON-compatible value.
pub(crate) fn frozen<'py>(
    value: &Bound<'py, PyAny>,
    path: &[PathSegment],
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    if scalar(value) {
        return Ok(value.clone());
    }
    if value.is_instance_of::<PyFloat>() {
        if !value.extract::<f64>()?.is_finite() {
            return Err(validation_error(
                py,
                &format!("{} must not contain NaN or infinity", rendered_path(path)),
            ));
        }
        return Ok(value.clone());
    }
    if abc_mapping(value)? {
        let copied: Bound<'py, PyDict> = PyDict::new(py);
        for item in mapping_items(value)?.try_iter()? {
            let (key, item): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
            let Ok(name) = key.downcast::<PyString>() else {
                return Err(validation_error(
                    py,
                    &format!("{} keys must be strings", rendered_path(path)),
                ));
            };
            if scalar(&item) {
                copied.set_item(&key, item)?;
                continue;
            }
            let nested: Vec<PathSegment> = extended(path, PathSegment::Key(name.extract()?));
            copied.set_item(&key, frozen(&item, &nested)?)?;
        }
        return imported(py, "types", "MappingProxyType")?.call1((copied,));
    }
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        let mut items: Vec<Bound<'py, PyAny>> = Vec::new();
        for (index, item) in value.try_iter()?.enumerate() {
            let nested: Vec<PathSegment> = extended(path, PathSegment::Index(index));
            items.push(frozen(&item?, &nested)?);
        }
        return Ok(PyTuple::new(py, items)?.into_any());
    }
    let type_name: String = value.get_type().getattr("__name__")?.extract()?;
    Err(validation_error(
        py,
        &format!(
            "{} contains non-JSON value of type {type_name}",
            rendered_path(path)
        ),
    ))
}

/// A frozen JSON value as a payload value.
pub(crate) fn payload_value(value: &Bound<'_, PyAny>) -> PyResult<PayloadValue> {
    if value.is_none() {
        return Ok(PayloadValue::Null);
    }
    if value.is_instance_of::<PyBool>() {
        return Ok(PayloadValue::Bool(value.is_truthy()?));
    }
    if value.is_instance_of::<PyInt>() {
        let saturated: i128 = match value.extract::<i128>() {
            Ok(number) => number,
            Err(_) if value.lt(0)? => i128::MIN,
            Err(_) => i128::MAX,
        };
        return Ok(PayloadValue::Integer {
            value: saturated,
            text: value.str()?.extract()?,
        });
    }
    if value.is_instance_of::<PyFloat>() {
        return Ok(PayloadValue::Float(value.extract()?));
    }
    if value.is_instance_of::<PyString>() {
        return Ok(PayloadValue::Text(value.extract()?));
    }
    if abc_mapping(value)? {
        return Ok(PayloadValue::Object(object_entries(value)?));
    }
    let items: Vec<PayloadValue> = value
        .try_iter()?
        .map(|item| payload_value(&item?))
        .collect::<PyResult<_>>()?;
    Ok(PayloadValue::Array(items))
}

/// The entries of a frozen JSON object in iteration order.
pub(crate) fn object_entries(value: &Bound<'_, PyAny>) -> PyResult<Vec<(String, PayloadValue)>> {
    mapping_items(value)?
        .try_iter()?
        .map(|item| {
            let (key, item): (String, Bound<'_, PyAny>) = item?.extract()?;
            Ok((key, payload_value(&item)?))
        })
        .collect()
}
