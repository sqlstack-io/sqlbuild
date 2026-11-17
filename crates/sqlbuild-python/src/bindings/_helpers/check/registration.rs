//! Register the native `sqb check` command and its result rendering.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::check::check_command;
use crate::bindings::_helpers::check::check_core;
use crate::bindings::_helpers::check::check_results;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    check_command::register(module)?;
    check_core::register(module)?;
    check_results::register(module)
}
