//! The compiled seed's `schema_entry`, as its Python attribute reads it.

use crate::resource_views::_helpers::seed_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The compiled seed's `schema_entry`, as its Python attribute reads it.
#[inline]
pub fn schema_entry<'py>(seed: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    seed_reads::schema_entry(seed)
}
