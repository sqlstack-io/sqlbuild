//! A native audit view's reference rows, or `None` for a hand-built audit record.

use crate::compiled_project::models::NativeCompiledProject;
use crate::resource_views::_helpers::audit_views::read_audit;
use crate::resource_views::models::NativeAuditInput;
use crate::resource_views::models::NativeCompiledAudit;
use pyo3::prelude::{Bound, Py, PyAny, PyResult, Python};
use sqlbuild_project::models::ReferenceRow;

/// A native audit view's reference rows, or `None` for a hand-built audit record.
pub fn audit_reference_rows(audit: &Bound<'_, PyAny>) -> PyResult<Option<Vec<ReferenceRow>>> {
    let py: Python<'_> = audit.py();
    let (owner, index): (&Py<NativeCompiledProject>, usize) =
        if let Ok(view) = audit.cast::<NativeCompiledAudit>() {
            (&view.get().owner, view.get().index)
        } else if let Ok(view) = audit.cast::<NativeAuditInput>() {
            (&view.get().owner, view.get().index)
        } else {
            return Ok(None);
        };
    read_audit(owner, index, py, |record, _| {
        Ok(Some(record.attached.references.clone()))
    })
}
