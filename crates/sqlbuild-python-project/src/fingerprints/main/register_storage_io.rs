//! Register the storage io functions and classes.

use crate::fingerprints::_helpers::storage_io::build_create_table_sql;
use crate::fingerprints::_helpers::storage_io::build_read_latest_sql;
use crate::fingerprints::_helpers::storage_io::read_latest_fingerprints;
use crate::fingerprints::_helpers::storage_io::write_fingerprint;
use crate::fingerprints::_helpers::storage_io::write_fingerprints;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the storage io functions and classes.
pub fn register_storage_io(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_create_table_sql, module)?)?;
    module.add_function(wrap_pyfunction!(build_read_latest_sql, module)?)?;
    module.add_function(wrap_pyfunction!(read_latest_fingerprints, module)?)?;
    module.add_function(wrap_pyfunction!(write_fingerprint, module)?)?;
    module.add_function(wrap_pyfunction!(write_fingerprints, module)?)?;
    Ok(())
}
