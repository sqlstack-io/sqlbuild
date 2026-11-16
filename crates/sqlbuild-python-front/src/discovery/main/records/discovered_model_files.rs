//! The (selected) model files; `extraction` is `(implicit alias columns, output locations)`.

use crate::discovery::_helpers::records::model_records;
use crate::discovery::models::ModelFileSelection;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// The (selected) model files; `extraction` is `(implicit alias columns, output locations)`.
#[inline]
pub fn discovered_model_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    extraction: (bool, bool),
    selection: ModelFileSelection<'py>,
) -> PyResult<Bound<'py, PyTuple>> {
    model_records::discovered_model_files(project_dir, extraction, selection)
}
