//! Read the source record behind a native source view, or `None` for any other object.

use crate::resource_views::_helpers::source_views;
use crate::resource_views::types::SourceRecord;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

/// Read the source record behind a native source view, or `None` for any other object.
#[inline]
pub fn read_source_view<'py, T>(
    view: &Bound<'py, PyAny>,
    read: impl FnOnce(&SourceRecord, Python<'py>) -> PyResult<T>,
) -> Option<PyResult<T>> {
    source_views::read_source_view(view, read)
}
