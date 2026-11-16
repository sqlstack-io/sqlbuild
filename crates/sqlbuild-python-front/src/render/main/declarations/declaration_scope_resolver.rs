//! Pair the serializable static index with original process-local declaration values.

use crate::render::_helpers::declarations::declaration_values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Pair the serializable static index with original process-local declaration values.
#[inline]
pub fn declaration_scope_resolver<'py>(
    discovered: &Bound<'py, PyAny>,
    scope_index: &Bound<'py, PyAny>,
    (loaded_macros, lookup): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    declaration_values::declaration_scope_resolver(discovered, scope_index, (loaded_macros, lookup))
}
