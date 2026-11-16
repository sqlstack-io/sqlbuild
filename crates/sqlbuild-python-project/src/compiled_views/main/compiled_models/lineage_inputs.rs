//! Each lineage output with the `(resource, column)` pairs it reads; empty without lineage.

use crate::compiled_views::_helpers::compiled_models::model_reads;
use crate::compiled_views::types::LineageInput;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Each lineage output with the `(resource, column)` pairs it reads; empty without lineage.
#[inline]
pub fn lineage_inputs(model: &Bound<'_, PyAny>) -> PyResult<Vec<LineageInput>> {
    model_reads::lineage_inputs(model)
}
