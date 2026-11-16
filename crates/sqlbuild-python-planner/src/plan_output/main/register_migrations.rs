//! Register the migrations functions and classes.

use crate::plan_output::_helpers::migrations::format_pre_build;
use crate::plan_output::_helpers::migrations::render_old_name;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the migrations functions and classes.
pub fn register_migrations(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(format_pre_build, module)?)?;
    module.add_function(wrap_pyfunction!(render_old_name, module)?)
}
