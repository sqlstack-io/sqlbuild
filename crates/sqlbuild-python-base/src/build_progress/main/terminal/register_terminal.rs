//! Add `stop_progress_spinners` and run it at exit, before finalization ends daemon threads.

use crate::build_progress::_helpers::terminal;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Add `stop_progress_spinners` and run it at exit, before finalization ends daemon threads.
#[inline]
pub fn register_terminal(module: &Bound<'_, PyModule>) -> PyResult<()> {
    terminal::register(module)
}
