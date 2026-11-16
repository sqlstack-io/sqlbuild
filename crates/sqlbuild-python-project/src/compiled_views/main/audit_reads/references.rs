//! The compiled audit's `references`, as its Python attribute reads it.

use crate::compiled_views::_helpers::audit_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The compiled audit's `references`, as its Python attribute reads it.
#[inline]
pub fn references<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    audit_reads::references(audit)
}
