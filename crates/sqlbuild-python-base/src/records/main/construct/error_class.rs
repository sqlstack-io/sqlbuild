//! The native error class `id` as registered, ignoring later attribute replacement.

use crate::records::_helpers::native_classes::construct;
use crate::records::types::ErrorId;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The native error class `id` as registered, ignoring later attribute replacement.
#[inline]
pub fn error_class(py: Python<'_>, id: ErrorId) -> PyResult<Bound<'_, PyAny>> {
    construct::error_class(py, id)
}
