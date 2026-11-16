//! The `ColumnTransformKind` member of a native transform code.

use crate::lineage::_helpers::lineage_facts::TRANSFORM_KINDS;
use crate::lineage::_helpers::lineage_facts::coded_member;
use pyo3::prelude::{Py, PyAny, PyResult, Python};
use sqlbuild_python_base::records::types::RecordId;

/// The `ColumnTransformKind` member of a native transform code.
pub fn transform_kind_member(py: Python<'_>, code: usize) -> PyResult<Py<PyAny>> {
    coded_member(py, (RecordId::ColumnTransformKind, &TRANSFORM_KINDS), code)
}
