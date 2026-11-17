//! Register the resource ids functions and classes.

use crate::result_output::_helpers::events::resource_ids::audit_resource_id;
use crate::result_output::_helpers::events::resource_ids::sql_test_resource_id;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the resource ids functions and classes.
pub fn register_resource_ids(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(sql_test_resource_id, module)?)?;
    module.add_function(wrap_pyfunction!(audit_resource_id, module)?)?;
    Ok(())
}
