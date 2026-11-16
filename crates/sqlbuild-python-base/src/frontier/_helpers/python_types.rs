//! The Python types the capture encoder distinguishes, and its scalar encodings.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBytes, PyBytesMethods, PyStringMethods, PyTuple};

use sqlbuild_core::json::models::{JsonInteger, JsonValue};

use crate::boundary::main::imported::imported;
use crate::frontier::_helpers::encoder::qualified_name;
use crate::records::main::fields::field::field as native_field;

/// The Python classes and functions the encoder checks values against.
#[derive(Debug)]
pub(crate) struct PythonTypes<'py> {
    pub(crate) enumeration: Bound<'py, PyAny>,
    pub(crate) mapping: Bound<'py, PyAny>,
    pub(crate) abstract_set: Bound<'py, PyAny>,
    pub(crate) sequence: Bound<'py, PyAny>,
    pub(crate) partial: Bound<'py, PyAny>,
    pub(crate) fields: Bound<'py, PyAny>,
    is_dataclass: Bound<'py, PyAny>,
    pure_path: Bound<'py, PyAny>,
    decimal: Bound<'py, PyAny>,
    dates: Bound<'py, PyTuple>,
    bytes_like: Bound<'py, PyTuple>,
    pattern: Bound<'py, PyAny>,
    module: Bound<'py, PyAny>,
    callables: Bound<'py, PyTuple>,
    sha256: Bound<'py, PyAny>,
}

impl<'py> PythonTypes<'py> {
    pub(crate) fn load(py: Python<'py>) -> PyResult<Self> {
        let types = |name: &str| imported(py, "types", name);
        Ok(Self {
            enumeration: imported(py, "enum", "Enum")?,
            mapping: imported(py, "collections.abc", "Mapping")?,
            abstract_set: imported(py, "collections.abc", "Set")?,
            sequence: imported(py, "collections.abc", "Sequence")?,
            partial: imported(py, "functools", "partial")?,
            fields: imported(py, "dataclasses", "fields")?,
            is_dataclass: imported(py, "dataclasses", "is_dataclass")?,
            pure_path: imported(py, "pathlib", "PurePath")?,
            decimal: imported(py, "decimal", "Decimal")?,
            dates: PyTuple::new(
                py,
                [
                    imported(py, "datetime", "date")?,
                    imported(py, "datetime", "time")?,
                ],
            )?,
            bytes_like: PyTuple::new(
                py,
                [
                    imported(py, "builtins", "bytes")?,
                    imported(py, "builtins", "bytearray")?,
                    imported(py, "builtins", "memoryview")?,
                ],
            )?,
            pattern: imported(py, "re", "Pattern")?,
            module: types("ModuleType")?,
            callables: PyTuple::new(
                py,
                [
                    types("FunctionType")?,
                    types("BuiltinFunctionType")?,
                    types("MethodType")?,
                    types("BuiltinMethodType")?,
                    imported(py, "builtins", "type")?,
                ],
            )?,
            sha256: imported(py, "hashlib", "sha256")?,
        })
    }

    /// Whether `value` is a dataclass instance rather than a dataclass.
    pub(crate) fn dataclass_instance_check(&self, value: &Bound<'py, PyAny>) -> PyResult<bool> {
        Ok(self.is_dataclass.call1((value,))?.is_truthy()?
            && !value.is_instance(&imported(value.py(), "builtins", "type")?)?)
    }

    /// `_encode_scalar`: paths, decimals, dates, bytes, patterns, modules and callables.
    pub(crate) fn scalar(&self, value: &Bound<'py, PyAny>) -> PyResult<Option<JsonValue>> {
        let text = |key: &str, text: String| {
            JsonValue::Object(vec![(key.to_owned(), JsonValue::String(text))])
        };
        if value.is_instance(&self.pure_path)? {
            return Ok(Some(text(
                "__path__",
                value.call_method0("as_posix")?.extract()?,
            )));
        }
        if value.is_instance(&self.decimal)? {
            return Ok(Some(text("__decimal__", value.str()?.to_str()?.to_owned())));
        }
        if value.is_instance(self.dates.as_any())? {
            return Ok(Some(text(
                "__datetime__",
                value.call_method0("isoformat")?.extract()?,
            )));
        }
        if value.is_instance(self.bytes_like.as_any())? {
            let data: Bound<'py, PyBytes> = imported(value.py(), "builtins", "bytes")?
                .call1((value,))?
                .downcast_into::<PyBytes>()?;
            let digest: String = self
                .sha256
                .call1((&data,))?
                .call_method0("hexdigest")?
                .extract()?;
            return Ok(Some(JsonValue::Object(vec![
                (
                    "__bytes__".to_owned(),
                    JsonValue::Integer(JsonInteger::from(data.as_bytes().len() as u64)),
                ),
                ("sha256".to_owned(), JsonValue::String(digest)),
            ])));
        }
        if value.is_instance(&self.pattern)? {
            return Ok(Some(text(
                "__pattern__",
                native_field(value, "pattern")?.str()?.to_str()?.to_owned(),
            )));
        }
        if value.is_instance(&self.module)? {
            return Ok(Some(text(
                "__module__",
                value.getattr("__name__")?.extract()?,
            )));
        }
        if value.is_instance(self.callables.as_any())? {
            return Ok(Some(text("__callable__", qualified_name(value)?)));
        }
        Ok(None)
    }
}
