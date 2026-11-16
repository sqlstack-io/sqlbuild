//! Select attached audits by their logical target and singular audits by dependency overlap.

use crate::planner_strategy::_helpers::audit_entries;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use std::collections::HashSet;

/// Select attached audits by their logical target and singular audits by dependency overlap.
#[inline]
pub fn audit_is_selected(
    audit: &Bound<'_, PyAny>,
    selected_keys: &HashSet<ObjectKey>,
) -> PyResult<bool> {
    audit_entries::audit_is_selected(audit, selected_keys)
}
