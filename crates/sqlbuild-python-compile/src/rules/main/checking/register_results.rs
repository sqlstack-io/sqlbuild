//! Register the results functions and classes.

use crate::rules::main::checking::format_rules_result::format_rules_result;
use crate::rules::main::checking::format_skipped_type_proof_rules::format_skipped_type_proof_rules;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the results functions and classes.
pub fn register_results(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(format_rules_result, module)?)?;
    module.add_function(wrap_pyfunction!(format_skipped_type_proof_rules, module)?)?;
    Ok(())
}
