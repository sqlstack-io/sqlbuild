//! Register the observations state functions and classes.

use crate::native_host::_helpers::observations_state::build_observations_create_table_sql;
use crate::native_host::_helpers::observations_state::build_observations_prune_sql;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the observations state functions and classes.
pub fn register_observations_state(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        build_observations_create_table_sql,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(build_observations_prune_sql, module)?)
}
