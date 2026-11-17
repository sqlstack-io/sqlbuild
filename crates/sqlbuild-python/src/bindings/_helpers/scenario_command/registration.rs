//! Register the native `sqb scenario test` and `sqb scenario capture` commands.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::scenario_command::capture_command;
use crate::bindings::_helpers::scenario_command::test_command;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    test_command::register(module)?;
    capture_command::register(module)
}
