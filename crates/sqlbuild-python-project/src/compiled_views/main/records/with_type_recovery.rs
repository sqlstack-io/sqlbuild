//! The model with its recovered bindings and its poisoned outputs' types forgotten.

use crate::compiled_views::_helpers::records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyFrozenSet;

/// The model with its recovered bindings and its poisoned outputs' types forgotten.
#[inline]
pub fn with_type_recovery<'py>(
    model: &Bound<'py, PyAny>,
    diagnostics: &Bound<'py, PyAny>,
    poisoned: &Bound<'py, PyFrozenSet>,
) -> PyResult<Bound<'py, PyAny>> {
    records::with_type_recovery(model, diagnostics, poisoned)
}
