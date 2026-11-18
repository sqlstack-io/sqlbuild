//! Register Dagster invocation assembly and event planning.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

use crate::bindings::_helpers::dagster_invocation::arguments;
use crate::bindings::_helpers::dagster_invocation::failures;
use crate::bindings::_helpers::dagster_invocation::results;
use crate::bindings::_helpers::dagster_invocation::translator;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(arguments::dagster_cli_arguments, module)?)?;
    module.add_function(wrap_pyfunction!(
        arguments::dagster_selector_log_lines,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(results::dagster_payload_results, module)?)?;
    module.add_function(wrap_pyfunction!(
        results::dagster_integration_results,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        results::dagster_selected_asset_results,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        results::dagster_execution_payload,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(failures::dagster_failure, module)?)?;
    module.add_function(wrap_pyfunction!(failures::dagster_live_failure, module)?)?;
    module.add_function(wrap_pyfunction!(
        translator::dagster_default_asset_key_path,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        translator::dagster_default_group_name,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(translator::dagster_default_tags, module)?)?;
    module.add_function(wrap_pyfunction!(
        translator::dagster_default_metadata,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        translator::dagster_default_description,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        translator::dagster_default_check_name,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        translator::dagster_default_check_metadata,
        module
    )?)
}
