//! The names of the inferred output columns, `None` when the shape is unknown.

use crate::compiled_views::_helpers::compiled_models::model_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The names of the inferred output columns, `None` when the shape is unknown.
#[inline]
pub fn inferred_names(model: &Bound<'_, PyAny>) -> PyResult<Option<Vec<String>>> {
    model_reads::inferred_names(model)
}
