//! Each inferred output column as `(name, type, nullability)`, `None` when none were inferred.

use crate::compiled_views::_helpers::model_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_project::types::InferredRow;

/// Each inferred output column as `(name, type, nullability)`, `None` when none were inferred.
#[inline]
pub fn inferred_rows(model: &Bound<'_, PyAny>) -> PyResult<Option<Vec<InferredRow>>> {
    model_reads::inferred_rows(model)
}
