//! Register the stage functions functions and classes.

use crate::analysis_session::_helpers::stage_functions;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the stage functions functions and classes.
#[inline]
pub fn register_stage_functions(module: &Bound<'_, PyModule>) -> PyResult<()> {
    stage_functions::register(module)
}
