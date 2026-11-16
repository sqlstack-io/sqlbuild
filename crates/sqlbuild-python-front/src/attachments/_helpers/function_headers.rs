//! Function header values as the native function attachment parses them.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods};
use pyo3::types::{
    PyDict, PyDictMethods, PyList, PyListMethods, PyString, PyStringMethods, PyTuple,
    PyTupleMethods,
};
use sqlbuild_attachments::functions::main::parse_function_header::parse_function_header;
use sqlbuild_attachments::functions::models::{FunctionHeader, FunctionLanguage, HeaderValue};

/// One function file's parsed header; non-string keys and `None` values are never read.
pub(crate) fn parsed_header(
    header_values: &Bound<'_, PyDict>,
    language: FunctionLanguage,
    relative_path: &str,
) -> FunctionHeader {
    let mut header: Vec<(String, HeaderValue)> = Vec::with_capacity(header_values.len());
    for (key, value) in header_values.iter() {
        let Some(name) = text_value(&key) else {
            continue;
        };
        if value.is_none() {
            continue;
        }
        header.push((name, header_value(&value)));
    }
    parse_function_header(&header, language, relative_path)
}

/// One header value.
fn header_value(value: &Bound<'_, PyAny>) -> HeaderValue {
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
fn text_value(value: &Bound<'_, PyAny>) -> Option<String> {
    match value.downcast::<PyString>() {
        Ok(text) => Some(text.to_string_lossy().into_owned()),
        Err(_) => None,
    }
}
