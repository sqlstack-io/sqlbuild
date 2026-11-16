//! Register the columns functions and classes.

use crate::planner_migrations::_helpers::column::columns::identical_renames;
use crate::planner_migrations::_helpers::column::columns::rename_hints;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the columns functions and classes.
pub fn register_columns(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(identical_renames, module)?)?;
    module.add_function(wrap_pyfunction!(rename_hints, module)?)
}
