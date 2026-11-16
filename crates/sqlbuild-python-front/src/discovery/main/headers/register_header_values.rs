//! Register the header values functions and classes.

use crate::discovery::_helpers::headers::header_values::model_output_column_locations;
use crate::discovery::_helpers::headers::header_values::parse_header_values;
use crate::discovery::_helpers::headers::header_values::prepare_model_header_tokens;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the header values functions and classes.
pub fn register_header_values(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(prepare_model_header_tokens, module)?)?;
    module.add_function(wrap_pyfunction!(parse_header_values, module)?)?;
    module.add_function(wrap_pyfunction!(model_output_column_locations, module)?)
}
