//! The model's MODEL header with physical columns adopted, or the ownership conflict.

use crate::refactor::_helpers::contract_edits;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The model's MODEL header with physical columns adopted, or the ownership conflict.
#[inline]
pub fn edit_contract_model(
    py: Python<'_>,
    model: &Bound<'_, PyAny>,
    physical_columns: &Bound<'_, PyAny>,
    overwrite: bool,
) -> PyResult<(Option<String>, Option<String>)> {
    contract_edits::edit_contract_model(py, model, physical_columns, overwrite)
}
