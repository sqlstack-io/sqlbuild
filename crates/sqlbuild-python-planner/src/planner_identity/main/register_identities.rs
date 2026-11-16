//! Register the identities functions and classes.

use crate::planner_identity::_helpers::identities::build_direct_model_version_identities;
use crate::planner_identity::main::merge_recomputed_model_changes::merge_recomputed_model_changes;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the identities functions and classes.
pub fn register_identities(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        build_direct_model_version_identities,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(merge_recomputed_model_changes, module)?)
}
