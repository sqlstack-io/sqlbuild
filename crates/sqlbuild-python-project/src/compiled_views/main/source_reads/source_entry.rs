//! The compiled source's `source_entry`, as its Python attribute reads it.

use crate::compiled_views::_helpers::source_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The compiled source's `source_entry`, as its Python attribute reads it.
#[inline]
pub fn source_entry<'py>(source: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    source_reads::source_entry(source)
}
