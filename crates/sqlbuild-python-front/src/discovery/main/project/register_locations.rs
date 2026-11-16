//! Register the locations functions and classes.

use crate::discovery::_helpers::project::locations::project_relative_path;
use crate::discovery::main::project::yaml_entry_line::yaml_entry_line;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the locations functions and classes.
pub fn register_locations(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(yaml_entry_line, module)?)?;
    module.add_function(wrap_pyfunction!(project_relative_path, module)?)?;
    Ok(())
}
