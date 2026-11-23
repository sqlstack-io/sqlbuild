//! Load all raw project inputs from disk before semantic resolution.

use crate::compile_driver::_helpers::stages::discovery::DiscoveryRequest;
use crate::compile_driver::_helpers::stages::discovery::assembled_inputs;
use crate::compile_driver::_helpers::stages::discovery::lifecycle;
use crate::frontier::main::produced::produced;
use crate::frontier::models::FrontierStage;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::compile_driver::constants::PROJECT_DISCOVERIES;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;
use sqlbuild_python_base::invocation_phases::constants::DISCOVERED_PHASE;
use sqlbuild_python_base::invocation_phases::main::phase_since::phase_since;
use sqlbuild_python_base::native_host::main::publish_invocation_phase::publish_invocation_phase;
use sqlbuild_python_front::discovery::models::SecretPolicy;
use std::time::Instant;

/// Load all raw project inputs from disk before semantic resolution.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn discover_project_inputs<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "discover_project_inputs",
        options,
        (
            &["project_dir"],
            &[
                "sql_analysis_enabled_override",
                "extract_output_column_locations",
                "secret_policy",
            ],
        ),
    )?;
    count_stage(py, PROJECT_DISCOVERIES, 1)?;
    let secret_policy: SecretPolicy = if arguments.passed("secret_policy")? {
        SecretPolicy::parse(&arguments.value("secret_policy")?.extract::<String>()?)?
    } else {
        SecretPolicy::Require
    };
    let request: DiscoveryRequest = DiscoveryRequest {
        project_dir: arguments.value("project_dir")?.unbind(),
        sql_analysis_enabled_override: arguments.value("sql_analysis_enabled_override")?.unbind(),
        extract_output_column_locations: !arguments.passed("extract_output_column_locations")?
            || arguments.flag("extract_output_column_locations")?,
        secret_policy,
    };
    let started: Instant = Instant::now();
    let discovered: Bound<'py, PyAny> = within(&lifecycle(py, "project_discovery")?, |_| {
        let assembled: Bound<'py, PyAny> =
            within(&lifecycle(py, "discovery_project_assembly")?, |_| {
                assembled_inputs(py, &request)
            })?;
        produced(FrontierStage::DiscoveredProjectInputs, assembled)
    })?;
    publish_invocation_phase(py, &phase_since(DISCOVERED_PHASE, started))?;
    Ok(discovered)
}
