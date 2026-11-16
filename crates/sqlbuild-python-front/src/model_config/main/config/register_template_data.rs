//! Register the template data functions and classes.

use crate::model_config::_helpers::config::template_data::expand_effective_vars;
use crate::model_config::_helpers::config::template_data::expand_template_data;
use crate::model_config::_helpers::config::template_data::record_template_reads;
use crate::model_config::_helpers::config::template_data::resolve_run_id;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the template data functions and classes.
pub fn register_template_data(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(expand_template_data, module)?)?;
    module.add_function(wrap_pyfunction!(expand_effective_vars, module)?)?;
    module.add_function(wrap_pyfunction!(record_template_reads, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_run_id, module)?)?;
    Ok(())
}
