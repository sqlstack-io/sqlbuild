//! Errors of migration planning: input lookup exceptions preserved across the pure proof callback.

use pyo3::{PyErr, exceptions::PyValueError};

pub(crate) struct ProofError(pub(crate) PyErr);

impl From<String> for ProofError {
    fn from(message: String) -> Self {
        Self(PyValueError::new_err(message))
    }
}
