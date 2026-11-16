//! Row record.

use crate::freshness::_helpers::state_io;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Row record.
#[inline]
pub fn row_record<'py>(row: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    state_io::row_record(row)
}
