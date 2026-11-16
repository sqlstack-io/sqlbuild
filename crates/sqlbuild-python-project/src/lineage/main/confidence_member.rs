//! The `ColumnLineageConfidence` member of a native confidence code.

use crate::lineage::_helpers::lineage_facts;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The `ColumnLineageConfidence` member of a native confidence code.
#[inline]
pub fn confidence_member(py: Python<'_>, code: usize) -> PyResult<Py<PyAny>> {
    lineage_facts::confidence_member(py, code)
}
