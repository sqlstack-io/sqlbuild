//! Function header values as the native function attachment parses them.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods};
use pyo3::types::{
    PyDict, PyDictMethods, PyList, PyListMethods, PyString, PyStringMethods, PyTuple,
    PyTupleMethods,
};
use sqlbuild_attachments::functions::models::HeaderValue;

/// One header value.
pub(crate) fn header_value(value: &Bound<'_, PyAny>) -> HeaderValue {
    if let Some(text) = text_value(value) {
        return HeaderValue::Text(text);
    }
    if let Ok(mapping) = value.downcast::<PyDict>() {
        return HeaderValue::Map(
            mapping
                .iter()
                .map(|(key, item)| (header_value(&key), header_value(&item)))
                .collect(),
        );
    }
    if let Ok(list) = value.downcast::<PyList>() {
        return HeaderValue::Sequence(list.iter().map(|item| header_value(&item)).collect());
    }
    if let Ok(tuple) = value.downcast::<PyTuple>() {
        return HeaderValue::Sequence(tuple.iter().map(|item| header_value(&item)).collect());
    }
    HeaderValue::Other
}

/// A `str` (or subclass) as text; lone surrogates, rejected at compile entry, read as U+FFFD.
pub(crate) fn text_value(value: &Bound<'_, PyAny>) -> Option<String> {
    match value.downcast::<PyString>() {
        Ok(text) => Some(text.to_string_lossy().into_owned()),
        Err(_) => None,
    }
}
