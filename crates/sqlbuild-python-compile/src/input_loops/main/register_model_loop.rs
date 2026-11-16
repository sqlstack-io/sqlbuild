//! Register the model loop functions and classes.

use crate::input_loops::models::NativeModelLoop;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the model loop functions and classes.
pub fn register_model_loop(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeModelLoop>()
}
