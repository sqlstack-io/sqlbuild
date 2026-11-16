//! Register the outcome objects functions and classes.

use crate::analysis_session::_helpers::outcome_objects::session_outcome_analysis;
use crate::analysis_session::main::dynamic_contract_proof::dynamic_contract_proof;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the outcome objects functions and classes.
pub fn register_outcome_objects(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(session_outcome_analysis, module)?)?;
    module.add_function(wrap_pyfunction!(dynamic_contract_proof, module)?)
}
