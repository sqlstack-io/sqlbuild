//! Register the native relation shape, coverage and declaration functions.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::semantic_shapes::coverage;
use crate::bindings::_helpers::semantic_shapes::shapes;
use crate::bindings::_helpers::semantic_shapes::source_bindings;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    shapes::register(module)?;
    coverage::register(module)?;
    source_bindings::register(module)
}
