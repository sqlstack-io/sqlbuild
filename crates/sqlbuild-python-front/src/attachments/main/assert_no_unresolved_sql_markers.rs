//! Fail if executable SQL still contains unresolved reference markers or an unclosed construct.

use crate::attachments::_helpers::reference_markers;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyString;

/// Fail if executable SQL still contains unresolved reference markers or an unclosed construct.
#[inline]
pub fn assert_no_unresolved_sql_markers(
    py: Python<'_>,
    sql: &Bound<'_, PyString>,
    context: &str,
) -> PyResult<()> {
    reference_markers::assert_no_unresolved_sql_markers(py, sql, context)
}
