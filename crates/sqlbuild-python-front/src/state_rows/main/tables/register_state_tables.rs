//! Register the state tables functions and classes.

use crate::state_rows::_helpers::tables::state_tables::render_create_sql;
use crate::state_rows::_helpers::tables::state_tables::render_insert_source_freshness_records_sql;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the state tables functions and classes.
pub fn register_state_tables(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(render_create_sql, module)?)?;
    module.add_function(wrap_pyfunction!(
        render_insert_source_freshness_records_sql,
        module
    )?)
}
