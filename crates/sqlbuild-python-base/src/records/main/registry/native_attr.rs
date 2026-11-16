//! The module attribute `id`, read now so a replaced attribute is seen.

use crate::records::_helpers::native_classes::registry;
use crate::records::types::NativeAttr;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The module attribute `id`, read now so a replaced attribute is seen.
#[inline]
pub fn native_attr(py: Python<'_>, id: NativeAttr) -> PyResult<Bound<'_, PyAny>> {
    registry::native(py, id)
}
