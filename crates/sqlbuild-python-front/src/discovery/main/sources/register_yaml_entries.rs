//! Register the yaml entries functions and classes.

use crate::discovery::_helpers::sources::yaml_entries::parse_schema_document;
use crate::discovery::_helpers::sources::yaml_entries::parse_sources_document;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the yaml entries functions and classes.
pub fn register_yaml_entries(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(parse_schema_document, module)?)?;
    module.add_function(wrap_pyfunction!(parse_sources_document, module)?)
}
