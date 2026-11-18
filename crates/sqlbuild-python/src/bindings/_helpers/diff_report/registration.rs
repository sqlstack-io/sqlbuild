//! Register the native `sqb diff` functions.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

use crate::bindings::_helpers::diff_report::command::execute_prepared_query_diff;
use crate::bindings::_helpers::diff_report::command::run_diff_command;
use crate::bindings::_helpers::diff_report::output::render_diff_error_json;
use crate::bindings::_helpers::diff_report::output::render_diff_output;
use crate::bindings::_helpers::diff_report::tolerances;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(render_diff_output, module)?)?;
    module.add_function(wrap_pyfunction!(render_diff_error_json, module)?)?;
    module.add_function(wrap_pyfunction!(run_diff_command, module)?)?;
    module.add_function(wrap_pyfunction!(execute_prepared_query_diff, module)?)?;
    tolerances::register(module)
}
