//! Register the native `sqb janitor` command, its planning and its plan rendering.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

use crate::bindings::_helpers::janitor::janitor_command;
use crate::bindings::_helpers::janitor::janitor_plan;
use crate::bindings::_helpers::janitor::janitor_planning::build_janitor_archive_name;
use crate::bindings::_helpers::janitor::janitor_planning::build_janitor_plan_binding;
use crate::bindings::_helpers::janitor::janitor_planning::build_query_diff_artifact_name;
use crate::bindings::_helpers::janitor::janitor_planning::parse_query_diff_artifact_name;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    janitor_command::register(module)?;
    janitor_plan::register(module)?;
    module.add_function(wrap_pyfunction!(build_janitor_plan_binding, module)?)?;
    module.add_function(wrap_pyfunction!(build_janitor_archive_name, module)?)?;
    module.add_function(wrap_pyfunction!(build_query_diff_artifact_name, module)?)?;
    module.add_function(wrap_pyfunction!(parse_query_diff_artifact_name, module)?)
}
