//! Python values as `json.dumps(value, sort_keys=True, ...)` writes them, with or without `default=str`.

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{
    PyBool, PyBoolMethods, PyDict, PyDictMethods, PyFloat, PyInt, PyList, PyString, PyTuple,
    PyTypeMethods,
};

use crate::records::main::fields::field::field as native_field;
use sqlbuild_core::json::main::dumps::dumps;
use sqlbuild_core::json::main::python_float_repr::python_float_repr;
use sqlbuild_core::json::models::{JsonDialect, JsonInteger, JsonValue, StdlibJsonOptions};

const BUILTINS_MODULE: &str = "builtins";

/// `json.dumps(value, sort_keys=True, separators=separators)`, with `default=str` when `default_str`.
pub(crate) fn sorted_dumps(
    value: &Bound<'_, PyAny>,
    separators: (&str, &str),
    default_str: bool,
) -> PyResult<String> {
    let encoded: JsonValue = Encoder::new(value.py(), default_str)?.encode(value)?;
    let dialect = JsonDialect::Stdlib(
        StdlibJsonOptions::new(None).with_separators(separators.0, separators.1),
    );
    dumps(&encoded, &dialect).map_err(|error| PyValueError::new_err(format!("{error:?}")))
}

/// `json.dumps(value, ensure_ascii=False, sort_keys=True, separators=separators)`.
pub(crate) fn unicode_sorted_dumps(
    value: &Bound<'_, PyAny>,
    separators: (&str, &str),
) -> PyResult<String> {
    let encoded: JsonValue = Encoder::new(value.py(), false)?.encode(value)?;
    let mut options: StdlibJsonOptions =
        StdlibJsonOptions::new(None).with_separators(separators.0, separators.1);
    options.ensure_ascii = false;
    dumps(&encoded, &JsonDialect::Stdlib(options))
        .map_err(|error| PyValueError::new_err(format!("{error:?}")))
}

/// The C encoder's type dispatch, with dictionaries ordered as `sorted(dict.items())`.
struct Encoder<'py> {
    default_str: bool,
    int_repr: Bound<'py, PyAny>,
    sorted: Bound<'py, PyAny>,
}

impl<'py> Encoder<'py> {
    fn new(py: Python<'py>, default_str: bool) -> PyResult<Self> {
        let builtins = py.import(BUILTINS_MODULE)?;
        Ok(Self {
            default_str,
            int_repr: native_field(&builtins, "int")?.getattr("__repr__")?,
            sorted: native_field(&builtins, "sorted")?,
        })
    }

    fn encode(&self, value: &Bound<'py, PyAny>) -> PyResult<JsonValue> {
        if value.is_instance_of::<PyString>() {
            return Ok(JsonValue::String(value.extract()?));
        }
        if value.is_none() {
            return Ok(JsonValue::Null);
        }
        if let Ok(flag) = value.downcast::<PyBool>() {
            return Ok(JsonValue::Bool(flag.is_true()));
        }
        if value.is_instance_of::<PyInt>() {
            return Ok(JsonValue::Integer(self.integer(value)?));
        }
        if value.is_instance_of::<PyFloat>() {
            return Ok(JsonValue::Float(value.extract()?));
        }
        if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
            return Ok(JsonValue::Array(
                value
                    .try_iter()?
                    .map(|item| self.encode(&item?))
                    .collect::<PyResult<_>>()?,
            ));
        }
        if let Ok(mapping) = value.downcast::<PyDict>() {
            return self.object(mapping);
        }
        if self.default_str {
            return Ok(JsonValue::String(value.str()?.extract()?));
        }
        Err(PyTypeError::new_err(format!(
            "Object of type {} is not JSON serializable",
            value.get_type().name()?
        )))
    }

    fn integer(&self, value: &Bound<'py, PyAny>) -> PyResult<JsonInteger> {
        let decimal: String = self.int_repr.call1((value,))?.extract()?;
        JsonInteger::parse(&decimal)
            .ok_or_else(|| PyValueError::new_err(format!("invalid integer text {decimal}")))
    }

    fn object(&self, mapping: &Bound<'py, PyDict>) -> PyResult<JsonValue> {
        let mut keys: Vec<Bound<'py, PyAny>> = mapping.keys().into_iter().collect();
        if keys.iter().all(|key| key.is_instance_of::<PyString>()) {
            let mut texts: Vec<(String, Bound<'py, PyAny>)> = keys
                .into_iter()
                .map(|key| Ok((key.extract::<String>()?, key)))
                .collect::<PyResult<_>>()?;
            texts.sort_by(|left, right| left.0.cmp(&right.0));
            keys = texts.into_iter().map(|(_, key)| key).collect();
        } else {
            keys = self
                .sorted
                .call1((mapping.keys(),))?
                .try_iter()?
                .collect::<PyResult<_>>()?;
        }
        let mut entries: Vec<(String, JsonValue)> = Vec::with_capacity(keys.len());
        for key in keys {
            let item: Bound<'py, PyAny> = mapping.as_any().get_item(&key)?;
            entries.push((self.key_text(&key)?, self.encode(&item)?));
        }
        Ok(JsonValue::Object(entries))
    }

    fn key_text(&self, key: &Bound<'py, PyAny>) -> PyResult<String> {
        if key.is_instance_of::<PyString>() {
            return key.extract();
        }
        if key.is_instance_of::<PyFloat>() {
            let number: f64 = key.extract()?;
            return Ok(if number.is_nan() {
                "NaN".to_owned()
            } else if number.is_infinite() {
                if number.is_sign_positive() {
                    "Infinity"
                } else {
                    "-Infinity"
                }
                .to_owned()
            } else {
                python_float_repr(number)
            });
        }
        if let Ok(flag) = key.downcast::<PyBool>() {
            return Ok(if flag.is_true() { "true" } else { "false" }.to_owned());
        }
        if key.is_none() {
            return Ok("null".to_owned());
        }
        if key.is_instance_of::<PyInt>() {
            return Ok(self.integer(key)?.as_str().to_owned());
        }
        Err(PyTypeError::new_err(format!(
            "keys must be str, int, float, bool or None, not {}",
            key.get_type().name()?
        )))
    }
}
