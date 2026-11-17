//! Register the resource costs state functions and classes.

use crate::resource_costs::_helpers::resource_costs_state::build_resource_costs_create_table_sql;
use crate::resource_costs::_helpers::resource_costs_state::build_resource_costs_prune_sql;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the resource costs state functions and classes.
pub fn register_resource_costs_state(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        build_resource_costs_create_table_sql,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(build_resource_costs_prune_sql, module)?)
}
