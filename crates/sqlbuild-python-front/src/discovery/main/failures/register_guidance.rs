//! Register the guidance functions and classes.

use crate::discovery::_helpers::failures::guidance::snippet_help;
use crate::discovery::_helpers::failures::guidance::sql_analysis_off_guidance;
use crate::discovery::main::failures::explicit_references_help::explicit_references_help;
use crate::discovery::main::failures::microbatch_guidance::microbatch_guidance;
use crate::discovery::main::failures::setting_help::setting_help;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the guidance functions and classes.
pub fn register_guidance(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(setting_help, module)?)?;
    module.add_function(wrap_pyfunction!(snippet_help, module)?)?;
    module.add_function(wrap_pyfunction!(sql_analysis_off_guidance, module)?)?;
    module.add_function(wrap_pyfunction!(microbatch_guidance, module)?)?;
    module.add_function(wrap_pyfunction!(explicit_references_help, module)?)?;
    Ok(())
}
