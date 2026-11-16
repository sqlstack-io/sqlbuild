//! The model's destination `CompiledRelationLocation`.

use crate::compiled_views::_helpers::model_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The model's destination `CompiledRelationLocation`.
#[inline]
pub fn destination<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    model_reads::destination(model)
}
