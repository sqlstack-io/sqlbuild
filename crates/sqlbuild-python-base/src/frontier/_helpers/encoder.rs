//! Encode a frontier object graph as canonical JSON, sharing large subtrees by digest.

use crate::boundary::main::utf16_units::utf16_units;

use pyo3::exceptions::{PyRecursionError, PyUnicodeEncodeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyString, PyStringMethods, PyTuple};

use sqlbuild_cache::hosted_shapes::main::captured_shape_name::captured_shape_name;
use sqlbuild_cache::stage_capture::constants::SKIPPED_SLOTS;
use sqlbuild_cache::stage_capture::main::capture_sort_key::capture_sort_key;
use sqlbuild_core::json::errors::JsonEmitError;
use sqlbuild_core::json::models::JsonValue;

use crate::records::main::fields::field::field as native_field;
use crate::records::main::fields::mapping_items::mapping_items;

/// An instance's attributes, by name in first-set order.
type Attributes<'py> = Vec<(String, Bound<'py, PyAny>)>;

/// Nesting at which encoding stops, as Python's recursion limit stops the Python encoder.
pub(crate) const MAX_DEPTH: usize = 900;
pub(crate) const RECURSION_MESSAGE: &str = "maximum recursion depth exceeded";
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
pub(crate) fn tagged(key: &str, value: JsonValue) -> JsonValue {
    JsonValue::Object(vec![(key.to_owned(), value)])
}

/// Set `key` in `entries`, keeping the position of a key already present, as a dict does.
pub(crate) fn with_entry<T>(
    mut entries: Vec<(String, T)>,
    key: String,
    value: T,
) -> Vec<(String, T)> {
    match entries.iter_mut().find(|(existing, _)| *existing == key) {
        Some(entry) => entry.1 = value,
        None => entries.push((key, value)),
    }
    entries
}

/// `table[name]` of a per-type table.
pub(crate) fn lookup<'a, T: Copy>(table: &'a [(&'a str, T)], name: &str) -> Option<T> {
    table
        .iter()
        .find(|(type_name, _)| *type_name == name)
        .map(|(_, value)| *value)
}

/// The `module:qualname` a captured class is written as, natively hosted shapes keeping their Python name.
pub(crate) fn captured_type_name(class: &Bound<'_, PyAny>) -> PyResult<String> {
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
    match module.cast::<PyString>() {
        Ok(module) => Ok(format!("{}:{name}", module.to_str()?)),
        Err(_) => Ok(name),
    }
}

/// `getattr(value, name, None)`.
pub(crate) fn attribute_or_none<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
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
pub(crate) fn text_value(text: &Bound<'_, PyString>) -> PyResult<JsonValue> {
    match text.to_str() {
        Ok(text) => Ok(JsonValue::String(text.to_owned())),
        Err(_) => utf16_units(text).map(JsonValue::Utf16),
    }
}

/// `items` sorted by their masked, key-sorted JSON text, as Python's `sorted` orders them.
pub(crate) fn sorted_items(items: Vec<JsonValue>) -> PyResult<Vec<JsonValue>> {
    let mut keyed: Vec<(String, JsonValue)> = items
        .into_iter()
        .map(|item| Ok((capture_sort_key(&item).map_err(emit_error)?, item)))
        .collect::<PyResult<_>>()?;
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(keyed.into_iter().map(|(_, item)| item).collect())
}

/// `_instance_attributes`: the instance `__dict__` and the slots set on `value`, or `None`.
pub(crate) fn instance_attributes<'py>(
    value: &Bound<'py, PyAny>,
) -> PyResult<Option<Attributes<'py>>> {
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
            let Ok(text) = name.cast::<PyString>() else {
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
