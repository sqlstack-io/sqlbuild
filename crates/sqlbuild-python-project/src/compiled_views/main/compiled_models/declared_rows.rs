//! Each declared column of the attached schema as `(name, type)`, `None` without a schema.

use crate::compiled_views::_helpers::compiled_models::model_reads;
use crate::compiled_views::types::DeclaredRow;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Each declared column of the attached schema as `(name, type)`, `None` without a schema.
#[inline]
pub fn declared_rows(model: &Bound<'_, PyAny>) -> PyResult<Option<Vec<DeclaredRow>>> {
    model_reads::declared_rows(model)
}
