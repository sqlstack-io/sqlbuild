//! A native audit view's reference rows, or `None` for a hand-built audit record.

use crate::compiled_views::_helpers::audit_views;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_project::models::ReferenceRow;

/// A native audit view's reference rows, or `None` for a hand-built audit record.
#[inline]
pub fn audit_reference_rows(audit: &Bound<'_, PyAny>) -> PyResult<Option<Vec<ReferenceRow>>> {
    audit_views::audit_reference_rows(audit)
}
