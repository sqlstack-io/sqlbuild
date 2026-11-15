//! The native record class or string enum `id` as registered, ignoring later attribute replacement.

use crate::records::_helpers::native_classes::construct;
use crate::records::types::RecordId;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The native record class or string enum `id` as registered, ignoring later attribute replacement.
#[inline]
pub fn record_class(py: Python<'_>, id: RecordId) -> PyResult<Bound<'_, PyAny>> {
    construct::record_class(py, id)
}
