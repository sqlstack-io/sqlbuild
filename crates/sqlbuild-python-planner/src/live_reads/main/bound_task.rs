//! A bounded warehouse read task over a planning connection.

use crate::live_reads::_helpers::bounded;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

/// A bounded warehouse read task running `sql` over a planning connection.
#[inline]
pub fn bound_task<'py>(
    py: Python<'py>,
    sql: String,
    (connection, execute): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyAny>> {
    bounded::bound_task(py, sql, (connection, execute))
}
