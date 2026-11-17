//! One function file's parsed header; non-string keys and `None` values are never read.

use crate::attachments::_helpers::function_headers::header_value;
use crate::attachments::_helpers::function_headers::text_value;
use pyo3::prelude::{Bound, PyAnyMethods};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_attachments::functions::main::parse_function_header::parse_function_header;
use sqlbuild_attachments::functions::models::{FunctionHeader, FunctionLanguage, HeaderValue};

/// One function file's parsed header; non-string keys and `None` values are never read.
pub fn parsed_header(
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
