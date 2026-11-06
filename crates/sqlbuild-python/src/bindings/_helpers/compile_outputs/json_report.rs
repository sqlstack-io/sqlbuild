//! Emit a command's JSON report as orjson would, or as `json.dumps` does when orjson rejects it.

use pyo3::exceptions::{PyRecursionError, PyTypeError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult};
use pyo3::types::{
    PyBool, PyBoolMethods, PyBytes, PyBytesMethods, PyDict, PyDictMethods, PyFloat, PyInt, PyList,
    PyString, PyStringMethods, PyTuple, PyTypeMethods,
};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::json::main::dumps::dumps;
use sqlbuild_core::json::models::{
    JsonDialect, JsonInteger, JsonValue, OrjsonOptions, StdlibJsonOptions,
};

use crate::bindings::_helpers::boundary::panics::{NativeCompilerError, compiler_guard};

const ORJSON_SURROGATE: &str = "str is not valid UTF-8: surrogates not allowed";
const ORJSON_WIDE_INTEGER: &str = "Integer exceeds 64-bit range";
const ORJSON_KEY: &str = "Dict key must be str";
const ORJSON_RECURSION: &str = "Recursion limit reached";
/// orjson refuses a dict, or a non-empty list or tuple, nested this many containers deep.
const ORJSON_MAX_CONTAINER_DEPTH: usize = 254;
/// CPython's default `sys.getrecursionlimit()`; `json.dumps(indent=2)` recurses once per level.
const PYTHON_RECURSION_LIMIT: usize = 1_000;
const PYTHON_RECURSION_MESSAGE: &str = "maximum recursion depth exceeded";
const SURROGATE_KEY: &str = "a JSON report key holds a lone surrogate, which report keys never do";
const ENCODE: &str = "encode";
const UTF16_LE: &str = "utf-16-le";
const SURROGATE_PASS: &str = "surrogatepass";
const INDENTED: OrjsonOptions = OrjsonOptions {
    indent_2: true,
    sort_keys: false,
};

/// What converting the report found besides its values.
#[derive(Default)]
struct Conversion {
    /// The `TypeError` orjson raises at the first value it rejects.
    orjson_error: Option<String>,
    /// The error `json.dumps` raises at the first value it cannot encode either.
    stdlib_error: Option<PyErr>,
    /// Containers entered above the value being converted.
    depth: usize,
}

/// The report as `orjson.dumps(report, option=OPT_INDENT_2)`, raising orjson's `TypeError`.
#[pyfunction]
pub(crate) fn emit_orjson_report(report: &Bound<'_, PyAny>) -> PyResult<String> {
    compiler_guard(|| {
        let mut conversion: Conversion = Conversion::default();
        let value: JsonValue = conversion.json_value(report)?;
        if let Some(error) = conversion.orjson_error {
            return Err(PyTypeError::new_err(error));
        }
        emitted(&value, &JsonDialect::Orjson(INDENTED))
    })
}

/// The report as orjson writes it, or as `json.dumps(indent=2)` does when orjson rejects it.
#[pyfunction]
pub(crate) fn emit_json_report(report: &Bound<'_, PyAny>) -> PyResult<String> {
    compiler_guard(|| {
        let mut conversion: Conversion = Conversion::default();
        let value: JsonValue = conversion.json_value(report)?;
        if conversion.orjson_error.is_none() {
            return emitted(&value, &JsonDialect::Orjson(INDENTED));
        }
        if let Some(error) = conversion.stdlib_error {
            return Err(error);
        }
        crate::bindings::_helpers::compile_context::delta::capture(|capture| {
            capture.stdlib_json = true;
        });
        emitted(
            &value,
            &JsonDialect::Stdlib(StdlibJsonOptions::new(Some(2))),
        )
    })
}

/// A Python value in orjson's supported domain, converted once at its data boundary.
pub(crate) fn orjson_value(value: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    let mut conversion: Conversion = Conversion::default();
    let converted: JsonValue = conversion.json_value(value)?;
    if let Some(error) = conversion.orjson_error {
        return Err(PyTypeError::new_err(error));
    }
    Ok(converted)
}

/// `orjson.dumps(value, option=OPT_SORT_KEYS)` for an already-native payload.
pub(crate) fn orjson_sorted(value: &JsonValue) -> PyResult<String> {
    emitted(
        value,
        &JsonDialect::Orjson(OrjsonOptions {
            indent_2: false,
            sort_keys: true,
        }),
    )
}

/// `json.dumps(value, indent=2)`, raising what `json.dumps` raises.
pub(crate) fn indented_json(value: &Bound<'_, PyAny>) -> PyResult<String> {
    let mut conversion: Conversion = Conversion::default();
    let converted: JsonValue = conversion.json_value(value)?;
    if let Some(error) = conversion.stdlib_error {
        return Err(error);
    }
    emitted(
        &converted,
        &JsonDialect::Stdlib(StdlibJsonOptions::new(Some(2))),
    )
}

/// `json.dumps(value)`, raising what `json.dumps` raises.
pub(crate) fn plain_json(value: &Bound<'_, PyAny>) -> PyResult<String> {
    let mut conversion: Conversion = Conversion::default();
    let converted: JsonValue = conversion.json_value(value)?;
    if let Some(error) = conversion.stdlib_error {
        return Err(error);
    }
    emitted(
        &converted,
        &JsonDialect::Stdlib(StdlibJsonOptions::new(None)),
    )
}

fn emitted(value: &JsonValue, dialect: &JsonDialect) -> PyResult<String> {
    dumps(value, dialect).map_err(|error| PyTypeError::new_err(format!("{error:?}")))
}

impl Conversion {
    fn orjson_rejects(&mut self, error: String) {
        self.orjson_error.get_or_insert(error);
    }

    fn stdlib_rejects(&mut self, error: PyErr) {
        self.stdlib_error.get_or_insert(error);
    }

    /// Enter a container under both recursion limits; `false` means do not descend into it.
    fn enter(&mut self, counted_by_orjson: bool) -> bool {
        if counted_by_orjson && self.depth >= ORJSON_MAX_CONTAINER_DEPTH {
            self.orjson_rejects(ORJSON_RECURSION.to_owned());
        }
        if self.depth >= PYTHON_RECURSION_LIMIT {
            self.stdlib_rejects(PyRecursionError::new_err(PYTHON_RECURSION_MESSAGE));
            return false;
        }
        self.depth += 1;
        true
    }

    fn json_value(&mut self, value: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
        if value.is_none() {
            return Ok(JsonValue::Null);
        }
        if let Ok(flag) = value.downcast::<PyBool>() {
            return Ok(JsonValue::Bool(flag.is_true()));
        }
        if value.is_instance_of::<PyInt>() {
            return self.integer(value);
        }
        if value.is_instance_of::<PyFloat>() {
            return Ok(JsonValue::Float(value.extract::<f64>()?));
        }
        if let Ok(text) = value.downcast::<PyString>() {
            return self.text(text);
        }
        if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
            return self.array(value);
        }
        if let Ok(mapping) = value.downcast::<PyDict>() {
            return self.object(mapping);
        }
        let name: String = value.get_type().name()?.to_string();
        self.orjson_rejects(format!("Type is not JSON serializable: {name}"));
        self.stdlib_rejects(PyTypeError::new_err(format!(
            "Object of type {name} is not JSON serializable"
        )));
        Ok(JsonValue::Null)
    }

    /// The string; one with a lone surrogate, which orjson rejects, as its UTF-16 code units.
    fn text(&mut self, text: &Bound<'_, PyString>) -> PyResult<JsonValue> {
        if let Ok(text) = text.to_str() {
            return Ok(JsonValue::String(text.to_owned()));
        }
        self.orjson_rejects(ORJSON_SURROGATE.to_owned());
        let encoded: Bound<'_, PyBytes> = text
            .call_method1(ENCODE, (UTF16_LE, SURROGATE_PASS))?
            .downcast_into::<PyBytes>()?;
        Ok(JsonValue::Utf16(
            encoded
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect(),
        ))
    }

    /// orjson encodes 64-bit integers; wider ones make it raise, so `json.dumps` writes them.
    fn integer(&mut self, value: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
        if let Ok(signed) = value.extract::<i64>() {
            return Ok(JsonValue::Integer(JsonInteger::from(signed)));
        }
        if let Ok(unsigned) = value.extract::<u64>() {
            return Ok(JsonValue::Integer(JsonInteger::from(unsigned)));
        }
        self.orjson_rejects(ORJSON_WIDE_INTEGER.to_owned());
        let decimal: String = value.str()?.to_str()?.to_owned();
        Ok(JsonInteger::parse(&decimal).map_or(JsonValue::Null, JsonValue::Integer))
    }

    fn array(&mut self, value: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
        if !self.enter(value.len()? > 0) {
            return Ok(JsonValue::Null);
        }
        let mut items: Vec<JsonValue> = Vec::new();
        for item in value.try_iter()? {
            items.push(self.json_value(&item?)?);
        }
        self.depth -= 1;
        Ok(JsonValue::Array(items))
    }

    fn object(&mut self, mapping: &Bound<'_, PyDict>) -> PyResult<JsonValue> {
        if !self.enter(true) {
            return Ok(JsonValue::Null);
        }
        let mut entries: Vec<(String, JsonValue)> = Vec::with_capacity(mapping.len());
        for (key, item) in mapping.iter() {
            let key: String = self.object_key(&key)?;
            let item: JsonValue = self.json_value(&item)?;
            entries.push((key, item));
        }
        self.depth -= 1;
        Ok(JsonValue::Object(entries))
    }

    /// A string key, or the text `json.dumps` writes for a scalar key orjson rejects.
    fn object_key(&mut self, key: &Bound<'_, PyAny>) -> PyResult<String> {
        if let Ok(text) = key.downcast::<PyString>() {
            return match self.text(text)? {
                JsonValue::String(text) => Ok(text),
                _ => Err(NativeCompilerError::new_err(SURROGATE_KEY)),
            };
        }
        self.orjson_rejects(ORJSON_KEY.to_owned());
        if key.is_none() {
            return Ok("null".to_owned());
        }
        if let Ok(flag) = key.downcast::<PyBool>() {
            return Ok(if flag.is_true() { "true" } else { "false" }.to_owned());
        }
        if key.is_instance_of::<PyInt>() {
            return Ok(key.str()?.to_str()?.to_owned());
        }
        if key.is_instance_of::<PyFloat>() {
            return float_key(key);
        }
        let name: String = key.get_type().name()?.to_string();
        self.stdlib_rejects(PyTypeError::new_err(format!(
            "keys must be str, int, float, bool or None, not {name}"
        )));
        Ok(String::new())
    }
}

/// `json.dumps` writes a float key as `float.__repr__`, or as `NaN` / `Infinity`.
pub(crate) fn float_key(key: &Bound<'_, PyAny>) -> PyResult<String> {
    let number: f64 = key.extract::<f64>()?;
    if number.is_nan() {
        return Ok("NaN".to_owned());
    }
    if number.is_infinite() {
        return Ok(if number.is_sign_positive() {
            "Infinity"
        } else {
            "-Infinity"
        }
        .to_owned());
    }
    Ok(key.repr()?.to_str()?.to_owned())
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(emit_orjson_report, module)?)?;
    module.add_function(wrap_pyfunction!(emit_json_report, module)?)
}
