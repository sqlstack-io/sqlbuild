//! Register the runs table functions.

use crate::native_host::_helpers::runs_state::build_runs_create_table_sql;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the runs table functions.
pub fn register_runs_state(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_runs_create_table_sql, module)?)
}
