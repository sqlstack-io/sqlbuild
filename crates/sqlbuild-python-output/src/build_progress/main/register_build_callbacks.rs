//! Register the build callbacks functions and classes.

use crate::build_progress::models::BuildProgressCallbacks;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the build callbacks functions and classes.
pub fn register_build_callbacks(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<BuildProgressCallbacks>()
}
