//! Register the model records functions and classes.

use crate::discovery::main::records::discover_model_files::discover_model_files;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the model records functions and classes.
pub fn register_model_records(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(discover_model_files, module)?)
}
