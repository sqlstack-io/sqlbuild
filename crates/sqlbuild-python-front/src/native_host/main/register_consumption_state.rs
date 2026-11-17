//! Register the consumption state functions and classes.

use crate::native_host::_helpers::consumption_state::build_consumption_create_table_sql;
use crate::native_host::_helpers::consumption_state::build_consumption_prune_sql;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the consumption state functions and classes.
pub fn register_consumption_state(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        build_consumption_create_table_sql,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(build_consumption_prune_sql, module)?)
}
