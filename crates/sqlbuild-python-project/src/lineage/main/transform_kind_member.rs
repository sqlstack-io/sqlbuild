//! The `ColumnTransformKind` member of a native transform code.

use crate::lineage::_helpers::lineage_facts;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The `ColumnTransformKind` member of a native transform code.
#[inline]
pub fn transform_kind_member(py: Python<'_>, code: usize) -> PyResult<Py<PyAny>> {
    lineage_facts::transform_kind_member(py, code)
}
