//! The audit as a `CompiledAudit`, projected from its owner when it is a native view.

use crate::compiled_views::_helpers::project::records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The audit as a `CompiledAudit`, projected from its owner when it is a native view.
#[inline]
pub fn compiled_audit_record<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    records::compiled_audit_record(audit)
}
