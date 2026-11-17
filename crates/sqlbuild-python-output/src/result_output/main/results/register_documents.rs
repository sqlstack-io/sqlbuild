//! Register the documents functions and classes.

use crate::result_output::_helpers::results::documents::format_audit_execution_json;
use crate::result_output::_helpers::results::documents::format_degradable_build_execution_json;
use crate::result_output::_helpers::results::documents::format_scenario_execution_json;
use crate::result_output::_helpers::results::documents::format_test_execution_json;
use crate::result_output::main::results::format_build_execution_json::format_build_execution_json;
use crate::result_output::main::results::format_clone_execution_json::format_clone_execution_json;
use crate::result_output::main::results::format_load_execution_json::format_load_execution_json;
use crate::result_output::main::results::format_scenario_snapshot_execution_json::format_scenario_snapshot_execution_json;
use crate::result_output::main::results::format_seed_execution_json::format_seed_execution_json;
use crate::result_output::main::results::write_execution_json_output::write_execution_json_output;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the documents functions and classes.
pub fn register_documents(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(format_build_execution_json, module)?)?;
    module.add_function(wrap_pyfunction!(format_seed_execution_json, module)?)?;
    module.add_function(wrap_pyfunction!(format_load_execution_json, module)?)?;
    module.add_function(wrap_pyfunction!(format_clone_execution_json, module)?)?;
    module.add_function(wrap_pyfunction!(format_test_execution_json, module)?)?;
    module.add_function(wrap_pyfunction!(format_audit_execution_json, module)?)?;
    module.add_function(wrap_pyfunction!(format_scenario_execution_json, module)?)?;
    module.add_function(wrap_pyfunction!(
        format_scenario_snapshot_execution_json,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        format_degradable_build_execution_json,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(write_execution_json_output, module)?)
}
