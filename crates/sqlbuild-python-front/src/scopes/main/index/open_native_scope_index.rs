//! Python's `open_native_scope_index`: the natively built index of discovered inputs.

use crate::scopes::_helpers::index::scope_index;
use crate::scopes::models::NativeScopeIndex;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Python's `open_native_scope_index`: the natively built index of discovered inputs.
#[inline]
pub fn open_native_scope_index(
    py: Python<'_>,
    discovered_inputs: &Bound<'_, PyAny>,
    loaded_macros: &Bound<'_, PyAny>,
    static_macros: Option<Bound<'_, PyAny>>,
) -> PyResult<NativeScopeIndex> {
    scope_index::open_native_scope_index(py, discovered_inputs, loaded_macros, static_macros)
}
