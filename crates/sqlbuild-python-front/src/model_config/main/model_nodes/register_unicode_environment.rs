//! Register the unicode environment functions and classes.

use crate::model_config::_helpers::model_nodes::unicode_environment::UnicodeEnvironment;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the unicode environment functions and classes.
pub fn register_unicode_environment(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<UnicodeEnvironment>()
}
