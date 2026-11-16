//! Register the json report functions and classes.

use crate::compile_outputs::_helpers::json_report;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the json report functions and classes.
#[inline]
pub fn register_json_report(module: &Bound<'_, PyModule>) -> PyResult<()> {
    json_report::register(module)
}
