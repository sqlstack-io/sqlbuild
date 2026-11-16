//! The compiled source's `key`, as its Python attribute reads it.

use crate::compiled_views::_helpers::source_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The compiled source's `key`, as its Python attribute reads it.
#[inline]
pub fn key<'py>(source: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    source_reads::key(source)
}
