//! Register the ceremonial select functions and classes.

use crate::attachments::_helpers::ceremonial_select;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the ceremonial select functions and classes.
#[inline]
pub fn register_ceremonial_select(module: &Bound<'_, PyModule>) -> PyResult<()> {
    ceremonial_select::register(module)
}
