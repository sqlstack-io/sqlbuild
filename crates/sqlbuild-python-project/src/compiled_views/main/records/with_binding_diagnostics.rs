//! The model with these final binding diagnostics.

use crate::compiled_views::_helpers::records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The model with these final binding diagnostics.
#[inline]
pub fn with_binding_diagnostics<'py>(
    model: &Bound<'py, PyAny>,
    diagnostics: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    records::with_binding_diagnostics(model, diagnostics)
}
