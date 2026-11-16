//! `dataclasses.replace(value, **changes)`.

use crate::compiled_views::_helpers::records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `dataclasses.replace(value, **changes)`.
#[inline]
pub fn replace<'py>(
    value: &Bound<'py, PyAny>,
    changes: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    records::replace(value, changes)
}
