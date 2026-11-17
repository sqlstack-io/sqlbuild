//! Register the nested progress functions and classes.

use crate::build_progress::models::NestedCommandProgressCallbacks;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the nested progress functions and classes.
pub fn register_nested_progress(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NestedCommandProgressCallbacks>()
}
