//! The member `name` of the native string enum `id`, looked up rather than constructed.

use crate::records::_helpers::native_classes::construct;
use crate::records::types::RecordId;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The member `name` of the native string enum `id`, looked up rather than constructed.
#[inline]
pub fn enum_member<'py>(py: Python<'py>, id: RecordId, name: &str) -> PyResult<Bound<'py, PyAny>> {
    construct::enum_member(py, id, name)
}
