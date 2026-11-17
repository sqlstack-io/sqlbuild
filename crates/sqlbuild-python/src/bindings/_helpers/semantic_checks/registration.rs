//! Register the native semantic completion functions.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::semantic_checks::help_catalogue;
use crate::bindings::_helpers::semantic_checks::resource_sql;
use crate::bindings::_helpers::semantic_checks::stage;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    resource_sql::register(module)?;
    help_catalogue::register(module)?;
    stage::register(module)
}
