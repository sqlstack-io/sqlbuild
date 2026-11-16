//! Register the detect functions and classes.

use crate::planner_changes::_helpers::detect::detect_changes_of;
use crate::planner_changes::_helpers::detect::schema_changes;
use crate::planner_changes::main::detect_model_changes::detect_model_changes;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the detect functions and classes.
pub fn register_detect(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(detect_changes_of, module)?)?;
    module.add_function(wrap_pyfunction!(detect_model_changes, module)?)?;
    module.add_function(wrap_pyfunction!(schema_changes, module)?)
}
