//! A resource's `destination` from whichever native view it is; any other object is read by attribute, counted.

use crate::resource_views::_helpers::resource_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A resource's `destination` from whichever native view it is; any other object is read by attribute, counted.
#[inline]
pub fn destination<'py>(resource: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    resource_reads::destination(resource)
}
