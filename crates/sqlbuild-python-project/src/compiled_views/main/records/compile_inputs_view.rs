//! The native owner's inputs view when it holds every model input; else a record.

use crate::compiled_project::models::NativeCompiledProject;
use crate::compiled_views::_helpers::records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// The native owner's inputs view when it holds every model input; else a record.
#[inline]
pub fn compile_inputs_view<'py>(
    fields: &Bound<'py, PyDict>,
    native_project: Option<Bound<'py, NativeCompiledProject>>,
) -> PyResult<Bound<'py, PyAny>> {
    records::compile_inputs_view(fields, native_project)
}
