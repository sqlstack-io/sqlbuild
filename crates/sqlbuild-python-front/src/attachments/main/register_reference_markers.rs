//! Register the reference markers functions and classes.

use crate::attachments::_helpers::reference_markers;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the reference markers functions and classes.
#[inline]
pub fn register_reference_markers(module: &Bound<'_, PyModule>) -> PyResult<()> {
    reference_markers::register(module)
}
