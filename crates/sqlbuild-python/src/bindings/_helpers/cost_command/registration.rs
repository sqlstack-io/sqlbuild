//! Register the native `sqb cost` command, its delayed refresh and `--since` parsing.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::cost_command::bounds;
use crate::bindings::_helpers::cost_command::command;
use crate::bindings::_helpers::cost_command::refresh;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    command::register(module)?;
    refresh::register(module)?;
    bounds::register(module)
}
