//! `CompileSqlReference` records of `rows`, built once into `cache`.

use crate::compiled_views::_helpers::model_inputs::model_input_view;
use crate::compiled_views::models::InputViewClasses;
use pyo3::prelude::Py;
use pyo3::prelude::PyResult;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyTuple;
use sqlbuild_project::models::ReferenceRow;

/// `CompileSqlReference` records of `rows`, built once into `cache`.
#[inline]
pub fn reference_tuple(
    classes: &InputViewClasses<'_>,
    rows: &[ReferenceRow],
    cache: &PyOnceLock<Py<PyTuple>>,
) -> PyResult<Py<PyTuple>> {
    model_input_view::reference_tuple(classes, rows, cache)
}
