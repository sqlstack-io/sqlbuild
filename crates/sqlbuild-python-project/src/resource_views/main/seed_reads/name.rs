//! The compiled seed's `name`, as its Python attribute reads it.

use crate::resource_views::_helpers::seed_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The compiled seed's `name`, as its Python attribute reads it.
#[inline]
pub fn name<'py>(seed: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    seed_reads::name(seed)
}
