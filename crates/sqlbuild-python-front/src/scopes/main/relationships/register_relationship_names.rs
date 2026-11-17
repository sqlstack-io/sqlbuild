//! Register the relationship names functions and classes.

use crate::scopes::_helpers::relationships::relationship_names::scope_expected_model_names;
use crate::scopes::_helpers::relationships::relationship_names::scope_test_ctes;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the relationship names functions and classes.
pub fn register_relationship_names(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(scope_expected_model_names, module)?)?;
    module.add_function(wrap_pyfunction!(scope_test_ctes, module)?)?;
    Ok(())
}
