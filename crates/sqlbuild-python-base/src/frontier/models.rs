//! Public records of frontier.

use crate::boundary::main::imported::imported;
use crate::frontier::_helpers::encoder::MAX_DEPTH;
use crate::frontier::_helpers::encoder::RECURSION_MESSAGE;
use crate::frontier::_helpers::encoder::attribute_or_none;
use crate::frontier::_helpers::encoder::captured_type_name;
use crate::frontier::_helpers::encoder::emit_error;
use crate::frontier::_helpers::encoder::instance_attributes;
use crate::frontier::_helpers::encoder::lookup;
use crate::frontier::_helpers::encoder::qualified_name;
use crate::frontier::_helpers::encoder::sorted_items;
use crate::frontier::_helpers::encoder::tagged;
use crate::frontier::_helpers::encoder::text_value;
use crate::frontier::_helpers::encoder::with_entry;
use crate::frontier::_helpers::python_types::PythonTypes;
use crate::records::main::fields::field::field as native_field;
use crate::records::main::fields::mapping_items::mapping_items;
use pyo3::exceptions::PyRecursionError;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyBool;
use pyo3::types::PyBoolMethods;
use pyo3::types::PyFloat;
use pyo3::types::PyInt;
use pyo3::types::PyList;
use pyo3::types::PyString;
use pyo3::types::PyStringMethods;
use pyo3::types::PyTuple;
use sqlbuild_cache::stage_capture::constants::CANONICAL_TYPES;
use sqlbuild_cache::stage_capture::constants::DECODED_SEQUENCES;
use sqlbuild_cache::stage_capture::constants::OMITTED_ATTRIBUTES;
use sqlbuild_cache::stage_capture::constants::PROJECTED_ATTRIBUTES;
use sqlbuild_cache::stage_capture::constants::RESERVED_KEYS;
use sqlbuild_cache::stage_capture::constants::SHARED_MARKER;
use sqlbuild_cache::stage_capture::constants::SHARED_MIN_CHARS;
use sqlbuild_cache::stage_capture::constants::UNORDERED_ATTRIBUTES;
use sqlbuild_cache::stage_capture::main::compact_capture_json::compact_capture_json;
use sqlbuild_cache::stage_capture::main::shared_node_digest::shared_node_digest;
use sqlbuild_cache::stage_capture::models::CaptureDocument;
use sqlbuild_core::json::models::JsonInteger;
use sqlbuild_core::json::models::JsonValue;
use std::collections::HashMap;
use std::collections::HashSet;

/// The capture encoder with the document its shared nodes are written to.
#[derive(Debug)]
pub struct CaptureEncoder<'py> {
    pub(crate) types: PythonTypes<'py>,
    pub(crate) active: HashSet<usize>,
    pub(crate) cycles: usize,
    pub(crate) encoded: HashMap<usize, (Bound<'py, PyAny>, JsonValue)>,
    pub(crate) digests: HashSet<String>,
    pub(crate) depth: usize,
    pub document: CaptureDocument,
    /// The document text written so far: the shared nodes, as Python streams them.
    pub written: String,
}

impl<'py> CaptureEncoder<'py> {
    pub fn new(py: Python<'py>) -> PyResult<Self> {
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
    pub fn encode(&mut self, value: &Bound<'py, PyAny>) -> PyResult<JsonValue> {
        if self.depth >= MAX_DEPTH {
            return Err(PyRecursionError::new_err(RECURSION_MESSAGE));
        }
        self.depth += 1;
        let encoded: PyResult<JsonValue> = self.encode_value(value);
        self.depth -= 1;
        encoded
    }

    pub(crate) fn encode_value(&mut self, value: &Bound<'py, PyAny>) -> PyResult<JsonValue> {
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
    pub(crate) fn share(&mut self, encoded: JsonValue) -> PyResult<JsonValue> {
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

    pub(crate) fn encode_container(&mut self, value: &Bound<'py, PyAny>) -> PyResult<JsonValue> {
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

    pub(crate) fn encode_items(&mut self, value: &Bound<'py, PyAny>) -> PyResult<JsonValue> {
        let mut items: Vec<JsonValue> = Vec::new();
        for item in value.try_iter()? {
            items.push(self.encode(&item?)?);
        }
        Ok(JsonValue::Array(items))
    }

    pub(crate) fn encode_dataclass(
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

    pub(crate) fn encode_mapping(&mut self, value: &Bound<'py, PyAny>) -> PyResult<JsonValue> {
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

    pub(crate) fn mapping_pairs(&mut self, value: &Bound<'py, PyAny>) -> PyResult<Vec<JsonValue>> {
        let mut pairs: Vec<JsonValue> = Vec::new();
        for item in mapping_items(value)?.try_iter()? {
            let item: Bound<'py, PyAny> = item?;
            let key: JsonValue = self.encode(&item.get_item(0)?)?;
            let encoded: JsonValue = self.encode(&item.get_item(1)?)?;
            pairs.push(JsonValue::Array(vec![key, encoded]));
        }
        Ok(pairs)
    }

    pub(crate) fn encode_instance(
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
