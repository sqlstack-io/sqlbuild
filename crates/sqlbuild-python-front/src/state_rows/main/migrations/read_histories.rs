//! The old-name view history of every recorded move in a schema that holds both state tables.

use crate::state_rows::_helpers::migrations::migration_io;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;
use pyo3::types::PyTuple;

/// The old-name view history of every recorded move in a schema that holds both state tables.
#[inline]
pub fn read_histories<'py>(request: Option<&Bound<'py, PyDict>>) -> PyResult<Bound<'py, PyTuple>> {
    migration_io::read_histories(request)
}
