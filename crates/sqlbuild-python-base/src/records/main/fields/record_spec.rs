//! The spec registered for `cls`, read from its class attribute.

use crate::records::_helpers::native_classes::fields;
use crate::records::models::RecordSpec;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::types::PyType;

/// The spec registered for `cls`, read from its class attribute.
#[inline]
pub fn record_spec(cls: &Bound<'_, PyType>) -> PyResult<&'static RecordSpec> {
    fields::record_spec(cls)
}
