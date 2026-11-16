//! The model's attached `SchemaModelEntry`, or `None`.

use crate::compiled_views::_helpers::model_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The model's attached `SchemaModelEntry`, or `None`.
#[inline]
pub fn schema_entry<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    model_reads::schema_entry(model)
}
