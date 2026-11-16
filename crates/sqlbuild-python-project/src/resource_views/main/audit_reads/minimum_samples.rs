//! The compiled audit's `minimum_samples`, as its Python attribute reads it.

use crate::resource_views::_helpers::audit_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The compiled audit's `minimum_samples`, as its Python attribute reads it.
#[inline]
pub fn minimum_samples<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    audit_reads::minimum_samples(audit)
}
