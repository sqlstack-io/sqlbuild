//! Register the yaml records functions and classes.

use crate::discovery::_helpers::records::yaml_records::load_yaml_document;
use crate::discovery::_helpers::records::yaml_records::load_yaml_files;
use crate::discovery::main::records::discover_schema_files::discover_schema_files;
use crate::discovery::main::records::discover_source_files::discover_source_files;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the yaml records functions and classes.
pub fn register_yaml_records(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(discover_source_files, module)?)?;
    module.add_function(wrap_pyfunction!(discover_schema_files, module)?)?;
    module.add_function(wrap_pyfunction!(load_yaml_files, module)?)?;
    module.add_function(wrap_pyfunction!(load_yaml_document, module)?)
}
