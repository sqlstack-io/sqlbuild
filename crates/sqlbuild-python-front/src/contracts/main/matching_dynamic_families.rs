//! Return declared dynamic families accepting a physical column name.

use crate::contracts::_helpers::spec_resolution;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// Return declared dynamic families accepting a physical column name.
#[inline]
pub fn matching_dynamic_families<'py>(
    families: &Bound<'py, PyAny>,
    column_name: &str,
) -> PyResult<Bound<'py, PyTuple>> {
    spec_resolution::matching_dynamic_families(families, column_name)
}
