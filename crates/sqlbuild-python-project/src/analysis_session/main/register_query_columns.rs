//! Register the query columns functions and classes.

use crate::analysis_session::_helpers::query_columns::fixture_projection_facts;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the query columns functions and classes.
pub fn register_query_columns(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(fixture_projection_facts, module)?)?;
    Ok(())
}
