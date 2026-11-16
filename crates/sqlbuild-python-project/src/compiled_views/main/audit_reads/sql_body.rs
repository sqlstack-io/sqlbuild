//! The compiled audit's `sql_body`, as its Python attribute reads it.

use crate::compiled_views::_helpers::audit_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The compiled audit's `sql_body`, as its Python attribute reads it.
#[inline]
pub fn sql_body<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    audit_reads::sql_body(audit)
}
