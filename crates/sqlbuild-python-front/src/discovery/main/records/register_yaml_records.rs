//! Register the yaml records functions and classes.

use crate::discovery::_helpers::records::yaml_records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the yaml records functions and classes.
#[inline]
pub fn register_yaml_records(module: &Bound<'_, PyModule>) -> PyResult<()> {
    yaml_records::register(module)
}
