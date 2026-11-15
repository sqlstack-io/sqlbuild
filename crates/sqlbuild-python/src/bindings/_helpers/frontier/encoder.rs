//! Encode a frontier object graph as canonical JSON, sharing large subtrees by digest.

use std::collections::{HashMap, HashSet};

use pyo3::exceptions::{PyRecursionError, PyUnicodeEncodeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{
    PyBool, PyBoolMethods, PyBytes, PyBytesMethods, PyFloat, PyInt, PyList, PyString,
    PyStringMethods, PyTuple,
};

use sqlbuild_cache::hosted_shapes::main::captured_shape_name::captured_shape_name;
use sqlbuild_cache::stage_capture::constants::{
    CANONICAL_TYPES, DECODED_SEQUENCES, OMITTED_ATTRIBUTES, PROJECTED_ATTRIBUTES, RESERVED_KEYS,
    SHARED_MARKER, SHARED_MIN_CHARS, SKIPPED_SLOTS, UNORDERED_ATTRIBUTES,
};
use sqlbuild_cache::stage_capture::main::capture_sort_key::capture_sort_key;
use sqlbuild_cache::stage_capture::main::compact_capture_json::compact_capture_json;
use sqlbuild_cache::stage_capture::main::shared_node_digest::shared_node_digest;
use sqlbuild_cache::stage_capture::models::CaptureDocument;
use sqlbuild_core::json::errors::JsonEmitError;
use sqlbuild_core::json::models::{JsonInteger, JsonValue};

use crate::bindings::_helpers::frontier::python_types::PythonTypes;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;

/// An instance's attributes, by name in first-set order.
type Attributes<'py> = Vec<(String, Bound<'py, PyAny>)>;

/// Nesting at which encoding stops, as Python's recursion limit stops the Python encoder.
const MAX_DEPTH: usize = 900;
const RECURSION_MESSAGE: &str = "maximum recursion depth exceeded";
const SURROGATE_MESSAGE: &str = "'utf-8' codec can't encode characters: surrogates not allowed";

/// The Python error a capture serializer failure stands for.
pub(crate) fn emit_error(error: JsonEmitError) -> PyErr {
    match error {
        JsonEmitError::LoneSurrogate => PyUnicodeEncodeError::new_err(SURROGATE_MESSAGE),
        JsonEmitError::NestingTooDeep => PyRecursionError::new_err(RECURSION_MESSAGE),
        other => PyValueError::new_err(format!("{other:?}")),
    }
}

/// `{"key": value}`.
fn tagged(key: &str, value: JsonValue) -> JsonValue {
    JsonValue::Object(vec![(key.to_owned(), value)])
}

/// Set `key` in `entries`, keeping the position of a key already present, as a dict does.
fn with_entry<T>(mut entries: Vec<(String, T)>, key: String, value: T) -> Vec<(String, T)> {
    match entries.iter_mut().find(|(existing, _)| *existing == key) {
        Some(entry) => entry.1 = value,
        None => entries.push((key, value)),
    }
    entries
}

/// `table[name]` of a per-type table.
fn lookup<'a, T: Copy>(table: &'a [(&'a str, T)], name: &str) -> Option<T> {
    table
        .iter()
        .find(|(type_name, _)| *type_name == name)
        .map(|(_, value)| *value)
}

/// The `module:qualname` a captured class is written as, natively hosted shapes keeping their Python name.
fn captured_type_name(class: &Bound<'_, PyAny>) -> PyResult<String> {
    let name: String = qualified_name(class)?;
    Ok(captured_shape_name(&name).map_or(name, str::to_owned))
}

/// `module:qualname` of a callable or class, as Python's `qualified_name` writes it.
pub(crate) fn qualified_name(value: &Bound<'_, PyAny>) -> PyResult<String> {
    let module: Bound<'_, PyAny> = attribute_or_none(value, "__module__")?;
    let mut name: Bound<'_, PyAny> = attribute_or_none(value, "__qualname__")?;
    if !name.is_truthy()? {
        name = attribute_or_none(value, "__name__")?;
    }
    if name.is_none() {
        return qualified_name(&value.get_type().into_any());
    }
    let name: String = name.str()?.to_str()?.to_owned();
    match module.downcast::<PyString>() {
        Ok(module) => Ok(format!("{}:{name}", module.to_str()?)),
        Err(_) => Ok(name),
    }
}

/// `getattr(value, name, None)`.
fn attribute_or_none<'py>(value: &Bound<'py, PyAny>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    match native_field(value, name) {
        Ok(attribute) => Ok(attribute),
        Err(error) if error.is_instance_of::<pyo3::exceptions::PyAttributeError>(py) => {
            Ok(py.None().into_bound(py))
        }
        Err(error) => Err(error),
    }
}

/// A Python `str` as JSON text; one holding a lone surrogate as its UTF-16 code units.
fn text_value(text: &Bound<'_, PyString>) -> PyResult<JsonValue> {
    if let Ok(text) = text.to_str() {
        return Ok(JsonValue::String(text.to_owned()));
    }
    let encoded: Bound<'_, PyBytes> = text
        .call_method1("encode", ("utf-16-le", "surrogatepass"))?
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

/// The capture encoder with the document its shared nodes are written to.
pub(crate) struct CaptureEncoder<'py> {
    types: PythonTypes<'py>,
    active: HashSet<usize>,
    cycles: usize,
    encoded: HashMap<usize, (Bound<'py, PyAny>, JsonValue)>,
    digests: HashSet<String>,
    depth: usize,
    pub(crate) document: CaptureDocument,
    /// The document text written so far: the shared nodes, as Python streams them.
    pub(crate) written: String,
}

impl<'py> CaptureEncoder<'py> {
    pub(crate) fn new(py: Python<'py>) -> PyResult<Self> {
        Ok(Self {
            types: PythonTypes::load(py)?,
            active: HashSet::new(),
            cycles: 0,
            encoded: HashMap::new(),
            digests: HashSet::new(),
            depth: 0,
            document: CaptureDocument::default(),
            written: String::new(),
        })
    }

    /// The JSON value of `value`, keeping insertion order and sorting only sets.
    pub(crate) fn encode(&mut self, value: &Bound<'py, PyAny>) -> PyResult<JsonValue> {
        if self.depth >= MAX_DEPTH {
            return Err(PyRecursionError::new_err(RECURSION_MESSAGE));
        }
        self.depth += 1;
        let encoded: PyResult<JsonValue> = self.encode_value(value);
        self.depth -= 1;
        encoded
    }

    fn encode_value(&mut self, value: &Bound<'py, PyAny>) -> PyResult<JsonValue> {
        if value.is_instance(&self.types.enumeration)? {
            let name: String = native_field(value, "name")?.str()?.to_str()?.to_owned();
            return Ok(tagged(
                "__enum__",
                JsonValue::String(format!(
                    "{}.{name}",
                    captured_type_name(&value.get_type().into_any())?
                )),
            ));
        }
        if let Ok(text) = value.downcast::<PyString>() {
            let encoded: JsonValue = text_value(text)?;
            return if value.len()? < SHARED_MIN_CHARS {
                Ok(encoded)
            } else {
                self.share(encoded)
            };
        }
        if value.is_none() {
            return Ok(JsonValue::Null);
        }
        if let Ok(flag) = value.downcast::<PyBool>() {
            return Ok(JsonValue::Bool(flag.is_true()));
        }
        if value.is_instance_of::<PyInt>() {
            let decimal: String = imported(value.py(), "builtins", "int")?
                .call_method1("__repr__", (value,))?
                .extract()?;
            return Ok(JsonInteger::parse(&decimal).map_or(JsonValue::Null, JsonValue::Integer));
        }
        if value.is_instance_of::<PyFloat>() {
            let number: f64 = value.extract()?;
            return Ok(if number.is_finite() {
                JsonValue::Float(number)
            } else {
                tagged(
                    "__float__",
                    JsonValue::String(value.repr()?.to_str()?.to_owned()),
                )
            });
        }
        if let Some(scalar) = self.types.scalar(value)? {
            return Ok(scalar);
        }
        let identity: usize = value.as_ptr() as usize;
        if self.active.contains(&identity) {
            self.cycles += 1;
            return Ok(tagged(
                "__cycle__",
                JsonValue::String(qualified_name(&value.get_type().into_any())?),
            ));
        }
        if let Some((_, known)) = self.encoded.get(&identity) {
            return Ok(known.clone());
        }
        let cycles: usize = self.cycles;
        self.active.insert(identity);
        let container: PyResult<JsonValue> = self.encode_container(value);
        let encoded: PyResult<JsonValue> = container.and_then(|container| self.share(container));
        self.active.remove(&identity);
        let encoded: JsonValue = encoded?;
        if self.cycles == cycles {
            self.encoded
                .insert(identity, (value.clone(), encoded.clone()));
        }
        Ok(encoded)
    }

    /// The shared-node marker for a large object, array or string, writing the node once.
    fn share(&mut self, encoded: JsonValue) -> PyResult<JsonValue> {
        if !matches!(
            encoded,
            JsonValue::Object(_) | JsonValue::Array(_) | JsonValue::String(_) | JsonValue::Utf16(_)
        ) {
            return Ok(encoded);
        }
        let text: String = compact_capture_json(&encoded).map_err(emit_error)?;
        if text.chars().count() < SHARED_MIN_CHARS {
            return Ok(encoded);
        }
        let digest: String = shared_node_digest(&text);
        if self.digests.insert(digest.clone()) {
            let chunk: String = self.document.shared_chunk(&digest, &text);
            self.written.push_str(&chunk);
        }
        Ok(tagged(SHARED_MARKER, JsonValue::String(digest)))
    }

    fn encode_container(&mut self, value: &Bound<'py, PyAny>) -> PyResult<JsonValue> {
        let type_name: String = captured_type_name(&value.get_type().into_any())?;
        if DECODED_SEQUENCES.contains(&type_name.as_str())
            && value.is_instance(&self.types.sequence)?
        {
            return self.encode_items(value);
        }
        if self.types.dataclass_instance_check(value)? {
            return self.encode_dataclass(value, type_name);
        }
        if value.is_instance(&self.types.mapping)? {
            return self.encode_mapping(value);
        }
        if value.is_instance(&self.types.abstract_set)? {
            let mut items: Vec<JsonValue> = Vec::new();
            for item in value.try_iter()? {
                items.push(self.encode(&item?)?);
            }
            return Ok(tagged("__set__", JsonValue::Array(sorted_items(items)?)));
        }
        if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
            return self.encode_items(value);
        }
        if value.is_instance(&self.types.partial)? {
            return Ok(JsonValue::Object(vec![
                (
                    "__partial__".to_owned(),
                    self.encode(&native_field(value, "func")?)?,
                ),
                (
                    "args".to_owned(),
                    self.encode(&native_field(value, "args")?)?,
                ),
                (
                    "keywords".to_owned(),
                    self.encode(&native_field(value, "keywords")?)?,
                ),
            ]));
        }
        self.encode_instance(value, type_name)
    }

    fn encode_items(&mut self, value: &Bound<'py, PyAny>) -> PyResult<JsonValue> {
        let mut items: Vec<JsonValue> = Vec::new();
        for item in value.try_iter()? {
            items.push(self.encode(&item?)?);
        }
        Ok(JsonValue::Array(items))
    }

    fn encode_dataclass(
        &mut self,
        value: &Bound<'py, PyAny>,
        type_name: String,
    ) -> PyResult<JsonValue> {
        let omitted: &[&str] = lookup(&OMITTED_ATTRIBUTES, &type_name).unwrap_or(&[]);
        let mut entries: Vec<(String, JsonValue)> =
            vec![("__type__".to_owned(), JsonValue::String(type_name))];
        for field in self.types.fields.call1((value,))?.try_iter()? {
            let name: String = native_field(&field?, "name")?.extract()?;
            if omitted.contains(&name.as_str()) {
                continue;
            }
            let item: Bound<'py, PyAny> = attribute_or_none(value, &name)?;
            let encoded: JsonValue = self.encode(&item)?;
            entries = with_entry(entries, name, encoded);
        }
        Ok(JsonValue::Object(entries))
    }

    fn encode_mapping(&mut self, value: &Bound<'py, PyAny>) -> PyResult<JsonValue> {
        let mut plain: bool = true;
        for key in value.try_iter()? {
            if !key?.is_instance_of::<PyString>() {
                plain = false;
                break;
            }
        }
        if plain {
            for key in value.try_iter()? {
                let key: Bound<'py, PyAny> = key?;
                if RESERVED_KEYS.contains(&key.downcast::<PyString>()?.to_cow()?.as_ref()) {
                    plain = false;
                    break;
                }
            }
        }
        if !plain {
            return Ok(tagged(
                "__mapping__",
                JsonValue::Array(self.mapping_pairs(value)?),
            ));
        }
        let mut entries: Vec<(String, JsonValue)> = Vec::new();
        for item in mapping_items(value)?.try_iter()? {
            let item: Bound<'py, PyAny> = item?;
            let key: String = item.get_item(0)?.str()?.to_str()?.to_owned();
            let encoded: JsonValue = self.encode(&item.get_item(1)?)?;
            entries = with_entry(entries, key, encoded);
        }
        Ok(JsonValue::Object(entries))
    }

    fn mapping_pairs(&mut self, value: &Bound<'py, PyAny>) -> PyResult<Vec<JsonValue>> {
        let mut pairs: Vec<JsonValue> = Vec::new();
        for item in mapping_items(value)?.try_iter()? {
            let item: Bound<'py, PyAny> = item?;
            let key: JsonValue = self.encode(&item.get_item(0)?)?;
            let encoded: JsonValue = self.encode(&item.get_item(1)?)?;
            pairs.push(JsonValue::Array(vec![key, encoded]));
        }
        Ok(pairs)
    }

    fn encode_instance(
        &mut self,
        value: &Bound<'py, PyAny>,
        type_name: String,
    ) -> PyResult<JsonValue> {
        let attributes: Option<Vec<(String, Bound<'py, PyAny>)>> =
            match lookup(&PROJECTED_ATTRIBUTES, &type_name) {
                Some(names) => Some(
                    names
                        .iter()
                        .map(|name| Ok(((*name).to_owned(), native_field(value, name)?)))
                        .collect::<PyResult<_>>()?,
                ),
                None => instance_attributes(value)?,
            };
        let Some(attributes) = attributes else {
            return Ok(tagged("__opaque__", JsonValue::String(type_name)));
        };
        let unordered: &[&str] = lookup(&UNORDERED_ATTRIBUTES, &type_name).unwrap_or(&[]);
        let omitted: &[&str] = lookup(&OMITTED_ATTRIBUTES, &type_name).unwrap_or(&[]);
        let canonical: String =
            lookup(&CANONICAL_TYPES, &type_name).map_or(type_name.clone(), str::to_owned);
        let mut entries: Vec<(String, JsonValue)> =
            vec![("__type__".to_owned(), JsonValue::String(canonical))];
        for (name, item) in attributes {
            if omitted.contains(&name.as_str()) {
                continue;
            }
            let encoded: JsonValue =
                if unordered.contains(&name.as_str()) && item.is_instance(&self.types.mapping)? {
                    tagged(
                        "__unordered_mapping__",
                        JsonValue::Array(sorted_items(self.mapping_pairs(&item)?)?),
                    )
                } else {
                    self.encode(&item)?
                };
            entries = with_entry(entries, name, encoded);
        }
        Ok(JsonValue::Object(entries))
    }
}

/// `items` sorted by their masked, key-sorted JSON text, as Python's `sorted` orders them.
fn sorted_items(items: Vec<JsonValue>) -> PyResult<Vec<JsonValue>> {
    let mut keyed: Vec<(String, JsonValue)> = items
        .into_iter()
        .map(|item| Ok((capture_sort_key(&item).map_err(emit_error)?, item)))
        .collect::<PyResult<_>>()?;
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(keyed.into_iter().map(|(_, item)| item).collect())
}

/// `_instance_attributes`: the instance `__dict__` and the slots set on `value`, or `None`.
fn instance_attributes<'py>(value: &Bound<'py, PyAny>) -> PyResult<Option<Attributes<'py>>> {
    let instance_dict: Bound<'py, PyAny> = attribute_or_none(value, "__dict__")?;
    let has_dict: bool = instance_dict.is_exact_instance_of::<pyo3::types::PyDict>()
        || instance_dict.is_instance_of::<pyo3::types::PyDict>();
    let mut attributes: Vec<(String, Bound<'py, PyAny>)> = Vec::new();
    if has_dict {
        for item in mapping_items(&instance_dict)?.try_iter()? {
            let item: Bound<'py, PyAny> = item?;
            let name: String = item.get_item(0)?.str()?.to_str()?.to_owned();
            attributes = with_entry(attributes, name, item.get_item(1)?);
        }
    }
    for cls in value.get_type().getattr("__mro__")?.try_iter()? {
        let slots: Bound<'py, PyAny> = cls?
            .getattr("__dict__")?
            .call_method1("get", ("__slots__", PyTuple::empty(value.py())))?;
        let names: Vec<Bound<'py, PyAny>> = if slots.is_instance_of::<PyString>() {
            vec![slots]
        } else {
            slots.try_iter()?.collect::<PyResult<_>>()?
        };
        for name in names {
            let Ok(text) = name.downcast::<PyString>() else {
                continue;
            };
            let text: String = text.to_str()?.to_owned();
            if SKIPPED_SLOTS.contains(&text.as_str()) || !value.hasattr(text.as_str())? {
                continue;
            }
            let item: Bound<'py, PyAny> = native_field(value, text.as_str())?;
            attributes = with_entry(attributes, text, item);
        }
    }
    if attributes.is_empty() && !has_dict {
        return Ok(None);
    }
    Ok(Some(attributes))
}
