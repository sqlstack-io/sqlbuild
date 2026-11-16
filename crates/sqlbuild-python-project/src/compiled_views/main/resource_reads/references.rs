//! A resource's `references` from whichever native view it is; any other object is read by attribute, counted.

use crate::compiled_views::_helpers::resource_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A resource's `references` from whichever native view it is; any other object is read by attribute, counted.
#[inline]
pub fn references<'py>(resource: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    resource_reads::references(resource)
}
