//! Register the explicit references functions and classes.

use crate::references::_helpers::explicit_references::explicit_reference_report;
use crate::references::main::hard_coded_relation_remedy::hard_coded_relation_remedy;
use crate::references::main::old_name_reference_message::old_name_reference_message;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the explicit references functions and classes.
pub fn register_explicit_references(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(explicit_reference_report, module)?)?;
    module.add_function(wrap_pyfunction!(hard_coded_relation_remedy, module)?)?;
    module.add_function(wrap_pyfunction!(old_name_reference_message, module)?)?;
    Ok(())
}
