//! Whether the attached schema declares dynamic column families.

use crate::compiled_views::_helpers::model_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Whether the attached schema declares dynamic column families.
#[inline]
pub fn dynamic_columns_declared(model: &Bound<'_, PyAny>) -> PyResult<bool> {
    model_reads::dynamic_columns_declared(model)
}
