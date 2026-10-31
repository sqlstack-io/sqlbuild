//! Loaded configuration values as the Python objects `tomllib` builds.

use crate::bindings::_helpers::records::fields::field as native_field;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, IntoPyObject, Py, PyAny, PyAnyMethods, PyModule, PyResult, Python};
use pyo3::types::{IntoPyDict, PyDict, PyDictMethods, PyList, PyListMethods};
use sqlbuild_config::models::{ConfigDate, ConfigDateTime, ConfigTime, ConfigValue};

type PyObject = Py<PyAny>;

/// Decimal digits per `int(str)` call, well below Python's default 4300-digit limit.
const BIG_INTEGER_CHUNK_DIGITS: usize = 1000;

fn object<'py, T: IntoPyObject<'py>>(py: Python<'py>, value: T) -> PyResult<PyObject> {
    value.into_py_any(py)
}

fn date(datetime: &Bound<'_, PyModule>, value: ConfigDate) -> PyResult<PyObject> {
    Ok(native_field(datetime, "date")?
        .call1((value.year, value.month, value.day))?
        .unbind())
}

fn time(datetime: &Bound<'_, PyModule>, value: ConfigTime) -> PyResult<PyObject> {
    Ok(native_field(datetime, "time")?
        .call1((value.hour, value.minute, value.second, value.microsecond))?
        .unbind())
}

fn date_time(datetime: &Bound<'_, PyModule>, value: ConfigDateTime) -> PyResult<PyObject> {
    let tzinfo: Option<Bound<'_, PyAny>> = match value.utc_offset_seconds {
        Some(seconds) => {
            let delta = native_field(datetime, "timedelta")?.call(
                (),
                Some(&[("seconds", seconds)].into_py_dict(datetime.py())?),
            )?;
            Some(native_field(datetime, "timezone")?.call1((delta,))?)
        }
        None => None,
    };
    let (date, time) = (value.date, value.time);
    Ok(native_field(datetime, "datetime")?
        .call1((
            date.year,
            date.month,
            date.day,
            time.hour,
            time.minute,
            time.second,
            time.microsecond,
            tzinfo,
        ))?
        .unbind())
}

/// Python's `int` of a decimal text of any length, built from chunks below the conversion limit.
fn big_integer(py: Python<'_>, text: &str) -> PyResult<PyObject> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(digits) => (true, digits),
        None => (false, text),
    };
    let int = py.import("builtins")?.getattr("int")?;
    let mut value: Bound<'_, PyAny> = int.call1((0,))?;
    for chunk in digits.as_bytes().chunks(BIG_INTEGER_CHUNK_DIGITS) {
        let chunk_text = std::str::from_utf8(chunk)
            .map_err(|error| pyo3::exceptions::PyValueError::new_err(error.to_string()))?;
        let scale = int.call1((10,))?.pow(chunk.len(), py.None())?;
        value = value.mul(scale)?.add(int.call1((chunk_text,))?)?;
    }
    if negative {
        value = value.neg()?;
    }
    Ok(value.unbind())
}

/// Build the Python object for one loaded value; mapping entries keep their document order.
pub(crate) fn config_value_to_python(py: Python<'_>, value: ConfigValue) -> PyResult<PyObject> {
    match value {
        ConfigValue::Null => Ok(py.None()),
        ConfigValue::Bool(flag) => object(py, flag),
        ConfigValue::Integer(number) => object(py, number),
        ConfigValue::BigInteger(text) => big_integer(py, &text),
        ConfigValue::Float(number) => object(py, number),
        ConfigValue::String(text) => object(py, text),
        ConfigValue::Date(value) => date(&py.import("datetime")?, value),
        ConfigValue::DateTime(value) => date_time(&py.import("datetime")?, value),
        ConfigValue::Time(value) => time(&py.import("datetime")?, value),
        ConfigValue::List(items) => {
            let list = PyList::empty(py);
            for item in items {
                list.append(config_value_to_python(py, item)?)?;
            }
            Ok(list.unbind().into_any())
        }
        ConfigValue::Map(entries) => {
            let mapping = PyDict::new(py);
            for (key, item) in entries {
                mapping.set_item(
                    config_value_to_python(py, key)?,
                    config_value_to_python(py, item)?,
                )?;
            }
            Ok(mapping.unbind().into_any())
        }
    }
}

/// JSON-backed configuration keeps integer widths and recursively constructs native Python values.
pub(crate) fn json_value_to_python<'py>(
    py: Python<'py>,
    value: &serde_json::Value,
) -> PyResult<Bound<'py, PyAny>> {
    use serde_json::Value;
    match value {
        Value::Null => Ok(py.None().into_bound(py)),
        Value::Bool(value) => value.into_bound_py_any(py),
        Value::String(value) => value.into_bound_py_any(py),
        Value::Number(value) => match (value.as_i64(), value.as_u64()) {
            (Some(value), _) => value.into_bound_py_any(py),
            (None, Some(value)) => value.into_bound_py_any(py),
            (None, None) => value.as_f64().unwrap_or(f64::NAN).into_bound_py_any(py),
        },
        Value::Array(items) => {
            let values = items
                .iter()
                .map(|item| json_value_to_python(py, item))
                .collect::<PyResult<Vec<_>>>()?;
            Ok(PyList::new(py, values)?.into_any())
        }
        Value::Object(entries) => {
            let table = PyDict::new(py);
            for (key, item) in entries {
                table.set_item(key, json_value_to_python(py, item)?)?;
            }
            Ok(table.into_any())
        }
    }
}
