//! Parse model headers.

use crate::sqltext::_helpers::model_headers;
use crate::sqltext::types::ParsedModelHeader;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Parse model headers.
#[inline]
pub fn parse_model_headers(
    py: Python<'_>,
    headers: Vec<String>,
) -> PyResult<Vec<ParsedModelHeader>> {
    model_headers::parse_model_headers(py, headers)
}
