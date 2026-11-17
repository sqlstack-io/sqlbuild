//! The audit as a `CompiledAudit`, projected from its owner when it is a native view.

use crate::compiled_views::_helpers::project::records::record;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use sqlbuild_python_base::records::types::RecordId;

/// The audit as a `CompiledAudit`, projected from its owner when it is a native view.
#[pyfunction]
pub fn compiled_audit_record<'py>(audit: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    record(audit, RecordId::CompiledAudit)
}
