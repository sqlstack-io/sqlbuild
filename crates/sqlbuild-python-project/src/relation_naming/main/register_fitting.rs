//! Register the fitting functions and classes.

use crate::relation_naming::_helpers::fitting::fit_artifact_logical_name;
use crate::relation_naming::_helpers::fitting::fit_auxiliary_relation_name;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the fitting functions and classes.
pub fn register_fitting(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(fit_artifact_logical_name, module)?)?;
    module.add_function(wrap_pyfunction!(fit_auxiliary_relation_name, module)?)?;
    Ok(())
}
