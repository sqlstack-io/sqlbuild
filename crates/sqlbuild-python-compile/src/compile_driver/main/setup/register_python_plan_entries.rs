//! Register the python plan entries functions and classes.

use crate::compile_driver::_helpers::setup::python_plan_entries::build_python_plan_entries;
use crate::compile_driver::main::setup::build_python_run_plan_outputs::build_python_run_plan_outputs;
use crate::compile_driver::main::setup::python_read_source_names_for::python_read_source_names_for;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the python plan entries functions and classes.
pub fn register_python_plan_entries(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(python_read_source_names_for, module)?)?;
    module.add_function(wrap_pyfunction!(build_python_run_plan_outputs, module)?)?;
    module.add_function(wrap_pyfunction!(build_python_plan_entries, module)?)
}
