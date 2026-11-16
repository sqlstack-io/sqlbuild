//! Register the metadata functions and classes.

use crate::planner_changes::_helpers::metadata::version_identity_metadata_payload;
use crate::planner_changes::_helpers::metadata::with_declared_columns_hash;
use crate::planner_changes::main::without_declared_columns_hash::without_declared_columns_hash;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the metadata functions and classes.
pub fn register_metadata(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(version_identity_metadata_payload, module)?)?;
    module.add_function(wrap_pyfunction!(with_declared_columns_hash, module)?)?;
    module.add_function(wrap_pyfunction!(without_declared_columns_hash, module)?)
}
