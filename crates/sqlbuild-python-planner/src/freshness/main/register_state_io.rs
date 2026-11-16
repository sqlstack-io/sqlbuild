//! Register the state io functions and classes.

use crate::freshness::_helpers::state_io::build_source_freshness_create_table_sql;
use crate::freshness::_helpers::state_io::build_source_freshness_read_latest_sql;
use crate::freshness::_helpers::state_io::read_latest_source_freshness;
use crate::freshness::_helpers::state_io::write_source_freshness_records;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the state io functions and classes.
pub fn register_state_io(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        build_source_freshness_read_latest_sql,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        build_source_freshness_create_table_sql,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(read_latest_source_freshness, module)?)?;
    module.add_function(wrap_pyfunction!(write_source_freshness_records, module)?)?;
    Ok(())
}
