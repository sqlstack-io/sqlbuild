//! The compiled source's `name`, as its Python attribute reads it.

use crate::compiled_views::_helpers::source_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The compiled source's `name`, as its Python attribute reads it.
#[inline]
pub fn name<'py>(source: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    source_reads::name(source)
}
