//! Register the reuse functions and classes.

use crate::compile_outputs::main::check_compile_reuse::check_compile_reuse;
use crate::compile_outputs::models::NativeReuseAttempt;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the reuse functions and classes.
pub fn register_reuse(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeReuseAttempt>()?;
    module.add_function(wrap_pyfunction!(check_compile_reuse, module)?)
}
