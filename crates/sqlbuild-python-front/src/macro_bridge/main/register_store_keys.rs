//! Register the store keys functions and classes.

use crate::macro_bridge::main::context_store_token::context_store_token;
use crate::macro_bridge::main::macro_store_token::macro_store_token;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the store keys functions and classes.
pub fn register_store_keys(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(macro_store_token, module)?)?;
    module.add_function(wrap_pyfunction!(context_store_token, module)?)
}
