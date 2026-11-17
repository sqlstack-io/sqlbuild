//! Register the template data functions and classes.

use crate::model_config::_helpers::config::template_data;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register the template data functions and classes.
#[inline]
pub fn register_template_data(module: &Bound<'_, PyModule>) -> PyResult<()> {
    template_data::register(module)
}
