//! The `ColumnLineageConfidence` member of a native confidence code.

use crate::lineage::_helpers::lineage_facts::CONFIDENCES;
use crate::lineage::_helpers::lineage_facts::coded_member;
use pyo3::prelude::{Py, PyAny, PyResult, Python};
use sqlbuild_python_base::records::types::RecordId;

/// The `ColumnLineageConfidence` member of a native confidence code.
pub fn confidence_member(py: Python<'_>, code: usize) -> PyResult<Py<PyAny>> {
    coded_member(py, (RecordId::ColumnLineageConfidence, &CONFIDENCES), code)
}
