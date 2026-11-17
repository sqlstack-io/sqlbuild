//! Register the native microbatch history projections and statements.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::microbatches::projection;
use crate::bindings::_helpers::microbatches::statements;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    projection::register(module)?;
    statements::register(module)
}
