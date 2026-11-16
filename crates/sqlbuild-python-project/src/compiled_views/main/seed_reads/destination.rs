//! The compiled seed's `destination`, as its Python attribute reads it.

use crate::compiled_views::_helpers::seed_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The compiled seed's `destination`, as its Python attribute reads it.
#[inline]
pub fn destination<'py>(seed: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    seed_reads::destination(seed)
}
