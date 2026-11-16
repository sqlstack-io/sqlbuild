//! A resource's `schema_entry` from whichever native view it is; any other object is read by attribute, counted.

use crate::resource_views::_helpers::resource_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A resource's `schema_entry` from whichever native view it is; any other object is read by attribute, counted.
#[inline]
pub fn schema_entry<'py>(resource: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    resource_reads::schema_entry(resource)
}
