//! `{item.<attribute>: item for item in items}`.

use crate::records::_helpers::native_classes::fields;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// `{item.<attribute>: item for item in items}`.
#[inline]
pub fn by_attribute<'py>(
    items: &Bound<'py, PyAny>,
    attribute: &str,
) -> PyResult<Bound<'py, PyDict>> {
    fields::by_attribute(items, attribute)
}
