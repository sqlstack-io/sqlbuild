//! One function file's parsed header; non-string keys and `None` values are never read.

use crate::attachments::_helpers::function_headers;
use pyo3::prelude::Bound;
use pyo3::types::PyDict;
use sqlbuild_attachments::functions::models::FunctionHeader;
use sqlbuild_attachments::functions::models::FunctionLanguage;

/// One function file's parsed header; non-string keys and `None` values are never read.
#[inline]
pub fn parsed_header(
    header_values: &Bound<'_, PyDict>,
    language: FunctionLanguage,
    relative_path: &str,
) -> FunctionHeader {
    function_headers::parsed_header(header_values, language, relative_path)
}
