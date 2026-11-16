//! Register the header declarations functions and classes.

use crate::discovery::_helpers::headers::header_declarations::model_header_declarations;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the header declarations functions and classes.
pub fn register_header_declarations(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(model_header_declarations, module)?)
}
