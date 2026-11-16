//! Register the project config functions and classes.

use crate::discovery::main::project::load_local_config_file::load_local_config_file;
use crate::discovery::main::project::load_project_config_file::load_project_config_file;
use crate::discovery::main::project::resolve_config_file_path::resolve_config_file_path;
use crate::discovery::main::project::validate_local_sql_analysis::validate_local_sql_analysis;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the project config functions and classes.
pub fn register_project_config(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(load_project_config_file, module)?)?;
    module.add_function(wrap_pyfunction!(load_local_config_file, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_config_file_path, module)?)?;
    module.add_function(wrap_pyfunction!(validate_local_sql_analysis, module)?)
}
