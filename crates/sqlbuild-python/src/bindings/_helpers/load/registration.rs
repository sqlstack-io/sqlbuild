//! Register the native `sqb load` command, its selection entries and progress reporter.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::load::{load_command, load_progress, load_selection};

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    load_command::register(module)?;
    load_progress::register(module)?;
    load_selection::register(module)
}
