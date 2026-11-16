//! Binding diagnostics for a batch of complete-schema validation requests, in request order.

use crate::analysis::_helpers::binding_catalog;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyTuple;

/// Binding diagnostics for a batch of complete-schema validation requests, in request order.
#[inline]
pub fn schema_validations<'py>(
    py: Python<'py>,
    requests: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    binding_catalog::schema_validations(py, requests)
}
