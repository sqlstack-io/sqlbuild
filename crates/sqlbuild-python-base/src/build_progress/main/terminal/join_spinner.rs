//! Join a stopped spinner thread briefly if it is still alive.

use crate::build_progress::_helpers::terminal;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Join a stopped spinner thread briefly if it is still alive.
#[inline]
pub fn join_spinner(thread: &Bound<'_, PyAny>) -> PyResult<()> {
    terminal::join_spinner(thread)
}
