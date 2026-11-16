//! Register the header columns functions and classes.

use crate::model_config::_helpers::headers::header_columns::parse_schema_columns;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the header columns functions and classes.
pub fn register_header_columns(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(parse_schema_columns, module)?)
}
