//! Register the column planning functions and classes.

use crate::planner_migrations::_helpers::column::column_planning::plan_column_migrations_of;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the column planning functions and classes.
pub fn register_column_planning(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(plan_column_migrations_of, module)?)
}
