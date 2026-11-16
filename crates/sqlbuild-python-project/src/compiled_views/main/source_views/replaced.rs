//! `dataclasses.replace(value, **changes)`.

use crate::compiled_views::_helpers::source_views;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;

/// `dataclasses.replace(value, **changes)`.
#[inline]
pub fn replaced(
    _py: Python<'_>,
    value: &Bound<'_, PyAny>,
    changes: &Bound<'_, PyDict>,
) -> PyResult<Py<PyAny>> {
    source_views::replaced(_py, value, changes)
}
