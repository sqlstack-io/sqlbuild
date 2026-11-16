//! Reject one identity unless it uses canonical lowercase snake_case.

use crate::contracts::_helpers::resource_identities;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyErr;
use pyo3::types::PyString;

/// Reject one identity unless it uses canonical lowercase snake_case.
#[inline]
pub fn validate_resource_identity(
    name: &Bound<'_, PyString>,
    kind: &str,
    path: &Bound<'_, PyAny>,
    private_identity: bool,
) -> Result<(), PyErr> {
    resource_identities::validate_resource_identity(name, kind, path, private_identity)
}
