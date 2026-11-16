//! The compiled audit's `definition_name`, as its Python attribute reads it.

use crate::resource_views::_helpers::audit_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The compiled audit's `definition_name`, as its Python attribute reads it.
#[inline]
pub fn definition_name<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    audit_reads::definition_name(audit)
}
