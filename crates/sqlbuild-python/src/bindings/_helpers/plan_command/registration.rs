//! Register the native `sqb plan` and `sqb rules` commands.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::plan_command::plan;
use crate::bindings::_helpers::plan_command::rules;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    plan::register(module)?;
    rules::register(module)
}
