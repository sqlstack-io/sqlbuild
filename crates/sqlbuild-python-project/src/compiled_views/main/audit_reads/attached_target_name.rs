//! The compiled audit's `attached_target_name`, as its Python attribute reads it.

use crate::compiled_views::_helpers::audit_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The compiled audit's `attached_target_name`, as its Python attribute reads it.
#[inline]
pub fn attached_target_name<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    audit_reads::attached_target_name(audit)
}
