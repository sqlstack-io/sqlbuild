//! Register the native `sqb query` and `sqb dag` commands.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::query_command::dag;
use crate::bindings::_helpers::query_command::query;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    query::register(module)?;
    dag::register(module)
}
