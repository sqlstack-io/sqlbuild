//! Value-based text of everything a stored macro call result depends on besides its call text.

use crate::macro_bridge::main::text::text;
use pyo3::create_exception;
use pyo3::exceptions::{PyRecursionError, PyUnicodeEncodeError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{
    PyBool, PyBytes, PyDict, PyFloat, PyFrozenSet, PyInt, PyList, PySet, PyString, PyStringMethods,
    PyTuple, PyTypeMethods,
};

use sqlbuild_cache::macro_store_keys::main::canonical_sort_text::canonical_sort_text;
use sqlbuild_core::json::models::{JsonInteger, JsonValue};

use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;

const VALUE_RENDERER_FIELD: &str = "_value_renderer";
/// Nesting at which a value counts as too deep to key, as Python's recursion limit stops it.
const MAX_DEPTH: usize = 900;
/// Stack left below which one more level of `canonical` moves onto a new stack segment.
const CANONICAL_RED_ZONE: usize = 128 * 1024;
/// Size of each stack segment `canonical` grows by.
const CANONICAL_STACK_GROWTH: usize = 2 * 1024 * 1024;

create_exception!(_native, UncacheableValue, pyo3::exceptions::PyException);

/// Whether `error` is one that makes a value unkeyable rather than a failure to report.
pub(crate) fn unstable(py: Python<'_>, error: &PyErr) -> bool {
    error.is_instance_of::<UncacheableValue>(py)
        || error.is_instance_of::<PyUnicodeEncodeError>(py)
        || error.is_instance_of::<PyRecursionError>(py)
}

/// `f"{kind.__module__}.{kind.__qualname__}"`.
fn type_name(kind: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(format!(
        "{}.{}",
        kind.getattr("__module__")?.str()?.to_cow()?,
        kind.getattr("__qualname__")?.str()?.to_cow()?
    ))
}

/// `[tag, value]`.
fn tagged(tag: String, value: JsonValue) -> JsonValue {
    JsonValue::Array(vec![JsonValue::String(tag), value])
}

/// The Python types `_canonical` distinguishes.
pub(crate) struct CanonicalTypes<'py> {
    enumeration: Bound<'py, PyAny>,
    decimal: Bound<'py, PyAny>,
    dates: Bound<'py, PyTuple>,
    timedelta: Bound<'py, PyAny>,
    pure_path: Bound<'py, PyAny>,
    mapping_proxy: Bound<'py, PyAny>,
    is_dataclass: Bound<'py, PyAny>,
    fields: Bound<'py, PyAny>,
    type_type: Bound<'py, PyAny>,
}

impl<'py> CanonicalTypes<'py> {
    pub(crate) fn load(py: Python<'py>) -> PyResult<Self> {
        Ok(Self {
            enumeration: imported(py, "enum", "Enum")?,
            decimal: imported(py, "decimal", "Decimal")?,
            dates: PyTuple::new(
                py,
                [
                    imported(py, "datetime", "date")?,
                    imported(py, "datetime", "time")?,
                ],
            )?,
            timedelta: imported(py, "datetime", "timedelta")?,
            pure_path: imported(py, "pathlib", "PurePath")?,
            mapping_proxy: imported(py, "types", "MappingProxyType")?,
            is_dataclass: imported(py, "dataclasses", "is_dataclass")?,
            fields: imported(py, "dataclasses", "fields")?,
            type_type: imported(py, "builtins", "type")?,
        })
    }

    /// `_canonical(value)`, on a stack that grows so nesting up to `MAX_DEPTH` cannot overflow it.
    pub(crate) fn canonical(&self, value: &Bound<'py, PyAny>, depth: usize) -> PyResult<JsonValue> {
        stacker::maybe_grow(CANONICAL_RED_ZONE, CANONICAL_STACK_GROWTH, || {
            self.canonical_level(value, depth)
        })
    }

    fn canonical_level(&self, value: &Bound<'py, PyAny>, depth: usize) -> PyResult<JsonValue> {
        if depth >= MAX_DEPTH {
            return Err(PyRecursionError::new_err(
                "maximum recursion depth exceeded",
            ));
        }
        let kind: Bound<'py, PyAny> = value.get_type().into_any();
        if value.is_instance(&self.enumeration)? {
            return Ok(JsonValue::Array(vec![
                JsonValue::String("enum".to_owned()),
                JsonValue::String(type_name(&kind)?),
                self.canonical(&native_field(value, "value")?, depth + 1)?,
            ]));
        }
        if value.is_none() {
            return Ok(JsonValue::Null);
        }
        if let Ok(flag) = value.downcast::<PyBool>() {
            return Ok(JsonValue::Bool(pyo3::types::PyBoolMethods::is_true(flag)));
        }
        if let Ok(string) = value.downcast::<PyString>() {
            return text(string);
        }
        if value.is_instance_of::<PyInt>() {
            return Ok(tagged(
                "int".to_owned(),
                JsonValue::String(value.str()?.to_cow()?.into_owned()),
            ));
        }
        if value.is_instance_of::<PyFloat>() {
            return Ok(tagged(
                "float".to_owned(),
                JsonValue::String(value.call_method0("hex")?.extract()?),
            ));
        }
        if value.is_instance(&self.decimal)? {
            return Ok(tagged(
                "decimal".to_owned(),
                JsonValue::String(value.str()?.to_cow()?.into_owned()),
            ));
        }
        if value.is_instance(self.dates.as_any())? {
            return Ok(tagged(
                type_name(&kind)?,
                JsonValue::String(value.call_method0("isoformat")?.extract()?),
            ));
        }
        if value.is_instance(&self.timedelta)? {
            let mut items: Vec<JsonValue> = vec![JsonValue::String("timedelta".to_owned())];
            for part in ["days", "seconds", "microseconds"] {
                items.push(JsonValue::Integer(JsonInteger::from(
                    native_field(value, part)?.extract::<i64>()?,
                )));
            }
            return Ok(JsonValue::Array(items));
        }
        if value.is_instance_of::<PyBytes>() {
            return Ok(tagged(
                "bytes".to_owned(),
                JsonValue::String(value.call_method0("hex")?.extract()?),
            ));
        }
        if value.is_instance(&self.pure_path)? {
            return Ok(tagged(
                "path".to_owned(),
                JsonValue::String(value.call_method0("as_posix")?.extract()?),
            ));
        }
        if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
            let mut items: Vec<JsonValue> = Vec::new();
            for item in value.try_iter()? {
                items.push(self.canonical(&item?, depth + 1)?);
            }
            return Ok(tagged(type_name(&kind)?, JsonValue::Array(items)));
        }
        if value.is_instance_of::<PySet>() || value.is_instance_of::<PyFrozenSet>() {
            let mut members: Vec<String> = Vec::new();
            for item in value.try_iter()? {
                let canonical: JsonValue = self.canonical(&item?, depth + 1)?;
                members.push(canonical_sort_text(&canonical).map_err(|error| {
                    pyo3::exceptions::PyValueError::new_err(format!("{error:?}"))
                })?);
            }
            members.sort();
            return Ok(tagged(
                type_name(&kind)?,
                JsonValue::Array(members.into_iter().map(JsonValue::String).collect()),
            ));
        }
        if value.is_instance_of::<PyDict>() || value.is_instance(&self.mapping_proxy)? {
            let mut pairs: Vec<JsonValue> = Vec::new();
            for item in mapping_items(value)?.try_iter()? {
                let item: Bound<'py, PyAny> = item?;
                pairs.push(JsonValue::Array(vec![
                    self.canonical(&item.get_item(0)?, depth + 1)?,
                    self.canonical(&item.get_item(1)?, depth + 1)?,
                ]));
            }
            return Ok(tagged("mapping".to_owned(), JsonValue::Array(pairs)));
        }
        if self.is_dataclass.call1((value,))?.is_truthy()? && !value.is_instance(&self.type_type)? {
            let mut fields: Vec<JsonValue> = Vec::new();
            for field in self.fields.call1((value,))?.try_iter()? {
                let name: Bound<'py, PyAny> = native_field(&field?, "name")?;
                let item: Bound<'py, PyAny> = value.getattr(name.downcast::<PyString>()?)?;
                fields.push(JsonValue::Array(vec![
                    text(name.downcast::<PyString>()?)?,
                    self.canonical(&item, depth + 1)?,
                ]));
            }
            return Ok(tagged(type_name(&kind)?, JsonValue::Array(fields)));
        }
        Err(UncacheableValue::new_err(type_name(&kind)?))
    }
}

/// `_canonical_context`: the context's type and each field, the value renderer by its type.
pub(crate) fn canonical_context<'py>(
    types: &CanonicalTypes<'py>,
    macro_context: &Bound<'py, PyAny>,
) -> PyResult<JsonValue> {
    let mut fields: Vec<JsonValue> = Vec::new();
    for field in types.fields.call1((macro_context,))?.try_iter()? {
        let name: Bound<'py, PyAny> = native_field(&field?, "name")?;
        let name_text: String = name.extract()?;
        let value: Bound<'py, PyAny> = macro_context.getattr(name_text.as_str())?;
        let canonical: JsonValue = if name_text == VALUE_RENDERER_FIELD {
            JsonValue::String(type_name(&value.get_type().into_any())?)
        } else {
            types.canonical(&value, 0)?
        };
        fields.push(JsonValue::Array(vec![
            text(name.downcast::<PyString>()?)?,
            canonical,
        ]));
    }
    Ok(JsonValue::Array(vec![
        JsonValue::String(type_name(&macro_context.get_type().into_any())?),
        JsonValue::Array(fields),
    ]))
}

/// An inaccessible declaration name, which is always a string, as JSON.
pub(crate) fn plain_json(value: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    match value.downcast::<PyString>() {
        Ok(string) => text(string),
        Err(_) => Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "Object of type {} is not JSON serializable",
            value.get_type().name()?
        ))),
    }
}
