//! Register the native `sqb load` command.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::load::load_command;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    load_command::register(module)
}
