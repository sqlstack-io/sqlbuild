//! Render audit SQL from unresolved markers, applying relation overrides first.

use crate::auditing::_helpers::render;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// Render audit SQL from unresolved markers, applying relation overrides first.
#[inline]
pub fn render_audit_sql(request: Option<&Bound<'_, PyDict>>) -> PyResult<String> {
    render::render_audit_sql(request)
}
