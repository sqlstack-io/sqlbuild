//! Simple top-level SELECT output names and their authored locations.

use crate::sqltext::_helpers::model_headers;
use crate::sqltext::models::ColumnLocation;
use pyo3::prelude::PyResult;

/// Simple top-level SELECT output names and their authored locations.
#[inline]
pub fn model_output_columns(
    contents: &str,
    implicit_aliases: bool,
) -> PyResult<Vec<ColumnLocation>> {
    model_headers::model_output_columns(contents, implicit_aliases)
}
