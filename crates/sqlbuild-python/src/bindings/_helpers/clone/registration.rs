//! Register the native `sqb clone` command.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::clone::clone_command;
use crate::bindings::_helpers::clone::clone_pipeline;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    clone_command::register(module)?;
    clone_pipeline::register(module)
}
