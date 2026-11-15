//! The member of the native string enum `id` whose value is `value`, as `Enum(value)` finds it.

use crate::records::_helpers::native_classes::construct;
use crate::records::types::RecordId;
use pyo3::IntoPyObject;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The member of the native string enum `id` whose value is `value`, as `Enum(value)` finds it.
#[inline]
pub fn enum_from_value<'py, V: IntoPyObject<'py>>(
    py: Python<'py>,
    id: RecordId,
    value: V,
) -> PyResult<Bound<'py, PyAny>> {
    construct::enum_from_value(py, id, value)
}
