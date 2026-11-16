//! Transfer each assembled model's typed analysis to its retained facts; return how many.

use crate::compiled_project::models::NativeCompiledProject;
use crate::compiled_views::_helpers::records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Transfer each assembled model's typed analysis to its retained facts; return how many.
#[inline]
pub fn record_model_analyses<'py>(
    project: &Bound<'py, NativeCompiledProject>,
    models: &Bound<'py, PyAny>,
) -> PyResult<usize> {
    records::record_model_analyses(project, models)
}
