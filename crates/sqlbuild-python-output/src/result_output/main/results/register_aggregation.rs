//! Register the aggregation functions and classes.

use crate::result_output::_helpers::results::aggregation::aggregate_build_result;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the aggregation functions and classes.
pub fn register_aggregation(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(aggregate_build_result, module)?)?;
    Ok(())
}
