//! Query cell values as display text and JSON values.

use pyo3::exceptions::{PyUnicodeEncodeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{
    PyBool, PyBoolMethods, PyDict, PyDictMethods, PyFloat, PyInt, PyList, PyListMethods, PyString,
    PyStringMethods, PyTuple,
};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::json::models::{JsonInteger, JsonValue};

use sqlbuild_python_base::boundary::main::imported::imported;

const NULL_TEXT: &str = "NULL";
const FIXED_POINT_FORMAT: &str = "f";

fn python_text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(value.str()?.to_string_lossy().into_owned())
}

/// Whether `value` is a `decimal.Decimal`.
pub(crate) fn decimal_instance(value: &Bound<'_, PyAny>) -> PyResult<bool> {
    value.is_instance(&imported(value.py(), "decimal", "Decimal")?)
}

/// `format(value, "f")`, Decimal's fixed-point text.
fn fixed_point(value: &Bound<'_, PyAny>) -> PyResult<String> {
    imported(value.py(), "builtins", "format")?
        .call1((value, FIXED_POINT_FORMAT))?
        .extract()
}

/// `NULL`, a Decimal in fixed-point notation, or the value's `str`.
pub(crate) fn display_text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    if value.is_none() {
        return Ok(NULL_TEXT.to_owned());
    }
    if decimal_instance(value)? {
        return fixed_point(value);
    }
    python_text(value)
}

/// The JSON `default` hook: Decimal in fixed-point notation, anything else as its `str`.
#[pyfunction]
pub(crate) fn query_json_default(value: &Bound<'_, PyAny>) -> PyResult<String> {
    if decimal_instance(value)? {
        return fixed_point(value);
    }
    python_text(value)
}

/// The text, or `None` for a `str` holding lone surrogates.
fn exact_text(text: &Bound<'_, PyString>) -> PyResult<Option<String>> {
    match text.to_str() {
        Ok(text) => Ok(Some(text.to_owned())),
        Err(error) if error.is_instance_of::<PyUnicodeEncodeError>(text.py()) => Ok(None),
        Err(error) => Err(error),
    }
}

/// The decimal integer, or `None` beyond Python's integer string conversion limit.
fn exact_integer(value: &Bound<'_, PyAny>) -> PyResult<Option<JsonValue>> {
    match value.str() {
        Ok(text) => Ok(JsonInteger::parse(&text.to_string_lossy()).map(JsonValue::Integer)),
        Err(error) if error.is_instance_of::<PyValueError>(value.py()) => Ok(None),
        Err(error) => Err(error),
    }
}

fn object_key(key: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if key.is_exact_instance_of::<PyString>() {
        return exact_text(key.downcast::<PyString>()?);
    }
    if key.is_none() {
        return Ok(Some("null".to_owned()));
    }
    if let Ok(flag) = key.downcast_exact::<PyBool>() {
        return Ok(Some(flag.is_true().to_string()));
    }
    if key.is_exact_instance_of::<PyInt>() {
        return Ok(Some(python_text(key)?));
    }
    Ok(None)
}

/// The value as the query JSON encoder writes it, or `None` when only Python's encoder can.
pub(crate) fn json_cell(value: &Bound<'_, PyAny>) -> PyResult<Option<JsonValue>> {
    if value.is_none() {
        return Ok(Some(JsonValue::Null));
    }
    if let Ok(flag) = value.downcast_exact::<PyBool>() {
        return Ok(Some(JsonValue::Bool(flag.is_true())));
    }
    if value.is_exact_instance_of::<PyInt>() {
        return exact_integer(value);
    }
    if value.is_exact_instance_of::<PyFloat>() {
        return Ok(Some(JsonValue::Float(value.extract()?)));
    }
    if value.is_exact_instance_of::<PyString>() {
        return Ok(exact_text(value.downcast::<PyString>()?)?.map(JsonValue::String));
    }
    if value.is_exact_instance_of::<PyList>() || value.is_exact_instance_of::<PyTuple>() {
        let mut items: Vec<JsonValue> = Vec::new();
        for item in value.try_iter()? {
            let Some(item) = json_cell(&item?)? else {
                return Ok(None);
            };
            items.push(item);
        }
        return Ok(Some(JsonValue::Array(items)));
    }
    if let Ok(mapping) = value.downcast_exact::<PyDict>() {
        let mut entries: Vec<(String, JsonValue)> = Vec::new();
        for (key, item) in mapping.iter() {
            let Some(key) = object_key(&key)? else {
                return Ok(None);
            };
            if entries.iter().any(|(existing, _)| *existing == key) {
                return Ok(None);
            }
            let Some(item) = json_cell(&item)? else {
                return Ok(None);
            };
            entries.push((key, item));
        }
        return Ok(Some(JsonValue::Object(entries)));
    }
    if value.is_instance_of::<PyString>()
        || value.is_instance_of::<PyInt>()
        || value.is_instance_of::<PyFloat>()
        || value.is_instance_of::<PyList>()
        || value.is_instance_of::<PyTuple>()
        || value.is_instance_of::<PyDict>()
    {
        return Ok(None);
    }
    Ok(Some(JsonValue::String(query_json_default(value)?)))
}

/// `json.dumps([dict(zip(columns, row)) ...], default=query_json_default) + "\n"`.
pub(crate) fn python_json_rows(
    py: Python<'_>,
    columns: &Bound<'_, PyAny>,
    rows: &[Vec<Bound<'_, PyAny>>],
) -> PyResult<String> {
    let names: Vec<Bound<'_, PyAny>> = columns.try_iter()?.collect::<PyResult<_>>()?;
    let objects: Bound<'_, PyList> = PyList::empty(py);
    for row in rows {
        let object: Bound<'_, PyDict> = PyDict::new(py);
        for (name, value) in names.iter().zip(row) {
            object.set_item(name, value)?;
        }
        objects.append(object)?;
    }
    let options: Bound<'_, PyDict> = PyDict::new(py);
    options.set_item("default", wrap_pyfunction!(query_json_default, py)?)?;
    let rendered: String = py
        .import("json")?
        .call_method("dumps", (objects,), Some(&options))?
        .extract()?;
    Ok(format!("{rendered}\n"))
}

/// The `ValueError` strict `zip` raises when a row and the columns differ in length.
pub(crate) fn strict_width(columns: usize, row: usize) -> PyResult<()> {
    match row.cmp(&columns) {
        std::cmp::Ordering::Less => Err(PyValueError::new_err(
            "zip() argument 2 is shorter than argument 1",
        )),
        std::cmp::Ordering::Greater => Err(PyValueError::new_err(
            "zip() argument 2 is longer than argument 1",
        )),
        std::cmp::Ordering::Equal => Ok(()),
    }
}
