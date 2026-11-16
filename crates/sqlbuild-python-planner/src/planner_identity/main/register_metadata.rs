//! Register the metadata functions and classes.

use crate::planner_identity::_helpers::metadata::build_version_identity_metadata_json;
use crate::planner_identity::_helpers::metadata::declared_columns_hash;
use crate::planner_identity::main::build_compiled_function_fingerprint_sql::build_compiled_function_fingerprint_sql;
use crate::planner_identity::main::build_function_local_hashes::build_function_local_hashes;
use crate::planner_identity::main::build_model_version_identity_metadata_json::build_model_version_identity_metadata_json;
use crate::planner_identity::main::build_seed_identity::build_seed_identity;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the metadata functions and classes.
pub fn register_metadata(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        build_version_identity_metadata_json,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        build_model_version_identity_metadata_json,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(declared_columns_hash, module)?)?;
    module.add_function(wrap_pyfunction!(build_function_local_hashes, module)?)?;
    module.add_function(wrap_pyfunction!(
        build_compiled_function_fingerprint_sql,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(build_seed_identity, module)?)
}
