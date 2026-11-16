//! Register the targets functions and classes.

use crate::compile_driver::_helpers::setup::targets::validate_managed_loader_target_isolation;
use crate::compile_driver::_helpers::setup::targets::validate_managed_write_schemas;
use crate::compile_driver::_helpers::setup::targets::validate_named_target_schema_strategy;
use crate::compile_driver::_helpers::setup::targets::validate_project_targets;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the targets functions and classes.
pub fn register_targets(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        validate_named_target_schema_strategy,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        validate_managed_loader_target_isolation,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(validate_project_targets, module)?)?;
    module.add_function(wrap_pyfunction!(validate_managed_write_schemas, module)?)
}
