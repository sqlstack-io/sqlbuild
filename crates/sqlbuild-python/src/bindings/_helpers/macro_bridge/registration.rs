//! Register the macro bridge's persistent store keys and module digests.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_front::macro_bridge::main::register_first_compile::register_first_compile;
use sqlbuild_python_front::macro_bridge::main::register_module_digests::register_module_digests;
use sqlbuild_python_front::macro_bridge::main::register_store_keys::register_store_keys;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_first_compile(module)?;
    register_module_digests(module)?;
    register_store_keys(module)
}
