//! Assemble selected audits in compiled-project order without querying lineage for an empty selection.

use crate::planner_strategy::_helpers::audit_entries;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;
use pyo3::types::PyList;

/// Assemble selected audits in compiled-project order without querying lineage for an empty selection.
#[inline]
pub fn build_selected_audit_entries<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyList>> {
    audit_entries::build_selected_audit_entries(request)
}
