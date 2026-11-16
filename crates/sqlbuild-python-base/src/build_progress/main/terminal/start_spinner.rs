//! Start a daemon thread running `owner._spin_until_stopped(event)`; return the event and thread.

use crate::build_progress::_helpers::terminal;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Start a daemon thread running `owner._spin_until_stopped(event)`; return the event and thread.
#[inline]
pub fn start_spinner<'py>(
    owner: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    terminal::start_spinner(owner)
}
