//! Register the python lifecycle functions and classes.

use crate::graph::main::python_nodes::build_python_sql_run_lifecycle::build_python_sql_run_lifecycle;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the python lifecycle functions and classes.
pub fn register_python_lifecycle(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_python_sql_run_lifecycle, module)?)
}
