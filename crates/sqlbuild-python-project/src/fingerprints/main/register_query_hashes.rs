//! Register the query hashes functions and classes.

use crate::fingerprints::_helpers::query_hashes::compute_function_definition_hash;
use crate::fingerprints::_helpers::query_hashes::normalize_query_sql;
use crate::fingerprints::main::compute_query_hash::compute_query_hash;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the query hashes functions and classes.
pub fn register_query_hashes(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(compute_query_hash, module)?)?;
    module.add_function(wrap_pyfunction!(compute_function_definition_hash, module)?)?;
    module.add_function(wrap_pyfunction!(normalize_query_sql, module)?)?;
    Ok(())
}
