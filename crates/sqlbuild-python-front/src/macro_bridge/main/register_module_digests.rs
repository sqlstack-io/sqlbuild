//! Register the module digests functions and classes.

use crate::macro_bridge::_helpers::module_digests::digest_module_files;
use crate::macro_bridge::_helpers::module_digests::module_digests_metadata;
use crate::macro_bridge::_helpers::module_digests::unchanged_module_digests;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the module digests functions and classes.
pub fn register_module_digests(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(digest_module_files, module)?)?;
    module.add_function(wrap_pyfunction!(module_digests_metadata, module)?)?;
    module.add_function(wrap_pyfunction!(unchanged_module_digests, module)?)
}
