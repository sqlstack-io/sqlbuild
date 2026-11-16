//! Python's `validate_scope_index`: raise one `ScopeValidationError` for its error diagnostics.

use crate::scopes::_helpers::checks::scope_validation;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyErr;

/// Python's `validate_scope_index`: raise one `ScopeValidationError` for its error diagnostics.
#[inline]
pub fn validate_scope_index(index: &Bound<'_, PyAny>) -> Result<(), PyErr> {
    scope_validation::validate_scope_index(index)
}
