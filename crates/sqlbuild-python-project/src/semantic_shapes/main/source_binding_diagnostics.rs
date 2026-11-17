//! `source_binding_diagnostics(*, project, columns, profile, selected_keys)`.

use crate::semantic_shapes::_helpers::source_bindings;
use pyo3::prelude::{Bound, PyResult};
use pyo3::types::{PyDict, PyTuple};

/// `source_binding_diagnostics(*, project, columns, profile, selected_keys)`.
#[inline]
pub fn source_binding_diagnostics<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    source_bindings::source_binding_diagnostics(request)
}
