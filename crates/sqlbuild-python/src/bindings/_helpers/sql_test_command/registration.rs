//! Register the native `sqb test` command, its progress labels and execution-limit parsing.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::sql_test_command::test_command;
use crate::bindings::_helpers::sql_test_command::timeouts;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    test_command::register(module)?;
    timeouts::register(module)
}
