//! The source's YAML declaration with physical columns adopted, or the ownership conflict.

use crate::refactor::_helpers::contract_edits;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The source's YAML declaration with physical columns adopted, or the ownership conflict.
#[inline]
pub fn edit_contract_source(
    source: &Bound<'_, PyAny>,
    physical_columns: &Bound<'_, PyAny>,
    overwrite: bool,
    contents: Option<String>,
) -> PyResult<(Option<String>, Option<String>)> {
    contract_edits::edit_contract_source(source, physical_columns, overwrite, contents)
}
