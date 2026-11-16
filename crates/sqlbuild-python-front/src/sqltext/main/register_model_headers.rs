//! Register the model headers functions and classes.

use crate::sqltext::_helpers::model_headers::model_header_column_locations;
use crate::sqltext::main::model_output_columns::model_output_columns;
use crate::sqltext::main::parse_model_headers::parse_model_headers;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the model headers functions and classes.
pub fn register_model_headers(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(parse_model_headers, module)?)?;
    module.add_function(wrap_pyfunction!(model_output_columns, module)?)?;
    module.add_function(wrap_pyfunction!(model_header_column_locations, module)?)?;
    Ok(())
}
