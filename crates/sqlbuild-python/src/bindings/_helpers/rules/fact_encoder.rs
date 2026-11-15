//! A simple deterministic encoding of custom-rule fact values, for fact digests.

use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::hash_map::Entry;

use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, PyTypeMethods,
    Python,
};
use pyo3::sync::PyOnceLock;
use pyo3::types::{
    PyBool, PyBoolMethods, PyDict, PyDictMethods, PyFloat, PyFrozenSet, PyInt, PyList, PySet,
    PyString, PyStringMethods, PyTuple, PyType,
};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_cache::digest::main::sha256_hex::sha256_hex;
use sqlbuild_core::json::main::dumps::dumps;
use sqlbuild_core::json::models::{JsonDialect, JsonValue, OrjsonOptions};

use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::record_spec::record_spec;
use sqlbuild_python_base::records::models::NativeRecord;

const VALUE_LABEL: &str = "value";
const ERROR_LABEL: &str = "error";
const ENUM_CLASS: (&str, &str) = ("enum", "Enum");
const INT_TAG: &str = "int";
const FLOAT_TAG: &str = "float";

static ENUM_TYPE: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

/// Why a fact cannot be digested: the value is not reproducible, or Python failed.
pub(crate) enum FactFailure {
    Unreproducible,
    Python(PyErr),
}

impl From<PyErr> for FactFailure {
    fn from(error: PyErr) -> Self {
        Self::Python(error)
    }
}

pub(crate) type Encoded<T> = Result<T, FactFailure>;

/// How the encoder writes values of one Python type.
#[derive(Clone)]
enum TypeKind {
    Record(Vec<&'static str>),
    Enum,
    Set,
    Other,
}

/// One classified type and the qualified name its records are tagged with.
struct TypeInfo {
    kind: TypeKind,
    qualified_name: String,
    _class: Py<PyType>,
}

/// Encode fact values; each record is written as its class and the digest of its fields.
pub(crate) struct FactEncoder {
    types: RefCell<HashMap<usize, TypeInfo>>,
    records: RefCell<HashMap<usize, (Py<PyAny>, JsonValue)>>,
    enums: RefCell<HashMap<usize, (Py<PyAny>, JsonValue)>>,
}

impl FactEncoder {
    pub(crate) fn new() -> Self {
        Self {
            types: RefCell::new(HashMap::new()),
            records: RefCell::new(HashMap::new()),
            enums: RefCell::new(HashMap::new()),
        }
    }

    /// The JSON standing for `value`.
    pub(crate) fn encode(&self, value: &Bound<'_, PyAny>) -> Encoded<JsonValue> {
        if value.is_none() {
            return Ok(JsonValue::Null);
        }
        if let Ok(text) = value.downcast::<PyString>() {
            return Ok(JsonValue::String(text.to_string_lossy().into_owned()));
        }
        if let Ok(flag) = value.downcast::<PyBool>() {
            return Ok(JsonValue::Bool(flag.is_true()));
        }
        if value.is_instance_of::<PyInt>() {
            return tagged(INT_TAG, value);
        }
        if value.is_instance_of::<PyFloat>() {
            return tagged(FLOAT_TAG, value);
        }
        if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
            let mut items: Vec<JsonValue> = Vec::new();
            for item in value.try_iter()? {
                items.push(self.encode(&item?)?);
            }
            return Ok(JsonValue::Array(items));
        }
        if let Ok(mapping) = value.downcast::<PyDict>() {
            let mut entries: Vec<(String, JsonValue)> = Vec::with_capacity(mapping.len());
            for (key, item) in mapping.iter() {
                entries.push((text_of(&self.encode(&key)?)?, self.encode(&item)?));
            }
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            return Ok(JsonValue::Array(
                entries
                    .into_iter()
                    .map(|(key, item)| JsonValue::Array(vec![JsonValue::String(key), item]))
                    .collect(),
            ));
        }
        self.object(value)
    }

    /// A record of `qualified_name` with these encoded field values.
    pub(crate) fn dataclass(
        &self,
        qualified_name: &str,
        fields: Vec<JsonValue>,
    ) -> Encoded<JsonValue> {
        Ok(JsonValue::Array(vec![
            JsonValue::String(qualified_name.to_owned()),
            JsonValue::String(digest_of(&JsonValue::Array(fields))?),
        ]))
    }

    /// One record from its field values, memoised by identity.
    pub(crate) fn record(
        &self,
        value: &Bound<'_, PyAny>,
        qualified_name: &str,
        fields: &[Bound<'_, PyAny>],
    ) -> Encoded<JsonValue> {
        let identity: usize = value.as_ptr() as usize;
        if let Some((_, encoded)) = self.records.borrow().get(&identity) {
            return Ok(encoded.clone());
        }
        let mut encoded: Vec<JsonValue> = Vec::with_capacity(fields.len());
        for field in fields {
            encoded.push(self.encode(field)?);
        }
        let encoded: JsonValue = self.dataclass(qualified_name, encoded)?;
        self.records
            .borrow_mut()
            .insert(identity, (value.clone().unbind(), encoded.clone()));
        Ok(encoded)
    }

    /// The qualified name records of `class` are tagged with.
    pub(crate) fn qualified_name(&self, class: &Bound<'_, PyType>) -> PyResult<String> {
        Ok(self.info(class)?.1)
    }

    /// A native record's field by name, or `None` when `value` is not a record with that field.
    pub(crate) fn field<'py>(
        &self,
        value: &Bound<'py, PyAny>,
        name: &str,
    ) -> PyResult<Option<Bound<'py, PyAny>>> {
        let (TypeKind::Record(names), _) = self.info(&value.get_type())? else {
            return Ok(None);
        };
        let Some(index) = names.iter().position(|field| *field == name) else {
            return Ok(None);
        };
        let record: &NativeRecord = value.downcast::<NativeRecord>()?.get();
        Ok(record
            .values
            .get(index)
            .map(|field| field.bind(value.py()).clone()))
    }

    fn object(&self, value: &Bound<'_, PyAny>) -> Encoded<JsonValue> {
        let class: Bound<'_, PyType> = value.get_type();
        let (kind, qualified_name) = self.info(&class)?;
        match kind {
            TypeKind::Record(_) => {
                let py: Python<'_> = value.py();
                let fields: Vec<Bound<'_, PyAny>> = value
                    .downcast::<NativeRecord>()
                    .map_err(PyErr::from)?
                    .get()
                    .values
                    .iter()
                    .map(|field| field.bind(py).clone())
                    .collect();
                self.record(value, &qualified_name, &fields)
            }
            TypeKind::Enum => {
                let identity: usize = value.as_ptr() as usize;
                if let Some((_, encoded)) = self.enums.borrow().get(&identity) {
                    return Ok(encoded.clone());
                }
                let encoded: JsonValue = JsonValue::Array(vec![
                    JsonValue::String(qualified_name),
                    self.encode(&native_field(value, "value")?)?,
                ]);
                self.enums
                    .borrow_mut()
                    .insert(identity, (value.clone().unbind(), encoded.clone()));
                Ok(encoded)
            }
            TypeKind::Set => {
                let mut texts: Vec<String> = Vec::new();
                for item in value.try_iter()? {
                    texts.push(text_of(&self.encode(&item?)?)?);
                }
                texts.sort();
                Ok(JsonValue::Array(
                    texts.into_iter().map(JsonValue::String).collect(),
                ))
            }
            TypeKind::Other => Ok(JsonValue::Array(vec![
                JsonValue::String(qualified_name),
                JsonValue::String(value.str()?.to_string_lossy().into_owned()),
            ])),
        }
    }

    /// The kind and qualified name of `class`, classified once.
    fn info(&self, class: &Bound<'_, PyType>) -> PyResult<(TypeKind, String)> {
        let identity: usize = class.as_ptr() as usize;
        if let Some(info) = self.types.borrow().get(&identity) {
            return Ok((info.kind.clone(), info.qualified_name.clone()));
        }
        let info: TypeInfo = classify(class)?;
        let found: (TypeKind, String) = (info.kind.clone(), info.qualified_name.clone());
        if let Entry::Vacant(vacant) = self.types.borrow_mut().entry(identity) {
            vacant.insert(info);
        }
        Ok(found)
    }
}

fn classify(class: &Bound<'_, PyType>) -> PyResult<TypeInfo> {
    let py: Python<'_> = class.py();
    let enum_type: &Py<PyAny> = ENUM_TYPE.get_or_try_init(py, || {
        Ok::<_, PyErr>(imported(py, ENUM_CLASS.0, ENUM_CLASS.1)?.unbind())
    })?;
    let kind: TypeKind = if class.is_subclass_of::<NativeRecord>()? {
        TypeKind::Record(
            record_spec(class)?
                .fields
                .iter()
                .map(|field| field.name)
                .collect(),
        )
    } else if class.is_subclass(enum_type.bind(py))? {
        TypeKind::Enum
    } else if class.is_subclass_of::<PySet>()? || class.is_subclass_of::<PyFrozenSet>()? {
        TypeKind::Set
    } else {
        TypeKind::Other
    };
    Ok(TypeInfo {
        kind,
        qualified_name: format!("{}.{}", class.module()?, class.qualname()?),
        _class: class.clone().unbind(),
    })
}

/// A number as its type tag and `repr`, so it never collides with a string of the same text.
fn tagged(tag: &str, value: &Bound<'_, PyAny>) -> Encoded<JsonValue> {
    Ok(JsonValue::Array(vec![
        JsonValue::String(tag.to_owned()),
        JsonValue::String(value.repr()?.to_string_lossy().into_owned()),
    ]))
}

/// The compact JSON text of `value`.
pub(crate) fn text_of(value: &JsonValue) -> Encoded<String> {
    dumps(value, &JsonDialect::Orjson(OrjsonOptions::default()))
        .map_err(|_unencodable| FactFailure::Unreproducible)
}

/// The SHA-256 hex digest of `value`'s JSON text.
pub(crate) fn digest_of(value: &JsonValue) -> Encoded<String> {
    Ok(sha256_hex(text_of(value)?.as_bytes()))
}

/// The digest of a fact that produced `value`, already encoded.
pub(crate) fn value_digest(value: JsonValue) -> Encoded<String> {
    digest_of(&JsonValue::Array(vec![
        JsonValue::String(VALUE_LABEL.to_owned()),
        value,
    ]))
}

/// The digest of a fact that raised an error of `kind` with `message`.
pub(crate) fn error_digest(kind: &str, message: &str) -> Encoded<String> {
    digest_of(&JsonValue::Array(vec![
        JsonValue::String(ERROR_LABEL.to_owned()),
        JsonValue::String(kind.to_owned()),
        JsonValue::String(message.to_owned()),
    ]))
}

/// The digest a rule host records for a live filesystem fact's value, or `None`.
#[pyfunction]
fn fact_value_digest(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    reproducible(FactEncoder::new().encode(value).and_then(value_digest))
}

/// The digest a rule host records for a live filesystem fact that raised `error`, or `None`.
#[pyfunction]
fn fact_error_digest(error: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    let kind: String = error.get_type().name()?.to_string();
    let message: String = error.str()?.to_string_lossy().into_owned();
    reproducible(error_digest(&kind, &message))
}

/// The digest, `None` when the value is not reproducible, or the Python failure.
pub(crate) fn reproducible<T>(encoded: Encoded<T>) -> PyResult<Option<T>> {
    match encoded {
        Ok(value) => Ok(Some(value)),
        Err(FactFailure::Unreproducible) => Ok(None),
        Err(FactFailure::Python(error)) => Err(error),
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(fact_value_digest, module)?)?;
    module.add_function(wrap_pyfunction!(fact_error_digest, module)?)
}
