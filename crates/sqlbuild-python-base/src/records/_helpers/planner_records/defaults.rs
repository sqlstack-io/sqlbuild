//! Factories for planner record field defaults.

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};

use crate::records::_helpers::native_classes::construct::{enum_member, new_record};
use crate::records::types::RecordId;

/// `ModelPlanEntry.backfill`'s default: a forward-only backfill.
pub(crate) fn forward_only_backfill(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    keywords.set_item(
        "action",
        enum_member(py, RecordId::BackfillAction, "FORWARD_ONLY")?,
    )?;
    new_record(py, RecordId::BackfillResult, (), Some(&keywords))
}
