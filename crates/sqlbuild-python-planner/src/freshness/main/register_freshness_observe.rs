//! Register the freshness observe functions and classes.

use crate::freshness::_helpers::freshness_observe::observe_source_freshness_json;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the freshness observe functions and classes.
pub fn register_freshness_observe(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(observe_source_freshness_json, module)?)?;
    Ok(())
}
