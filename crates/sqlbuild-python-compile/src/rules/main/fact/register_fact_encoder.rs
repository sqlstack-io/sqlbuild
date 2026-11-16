//! Register the fact encoder functions and classes.

use crate::rules::_helpers::fact::fact_encoder::fact_error_digest;
use crate::rules::_helpers::fact::fact_encoder::fact_value_digest;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the fact encoder functions and classes.
pub fn register_fact_encoder(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(fact_value_digest, module)?)?;
    module.add_function(wrap_pyfunction!(fact_error_digest, module)?)
}
