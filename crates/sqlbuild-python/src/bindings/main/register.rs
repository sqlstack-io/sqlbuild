use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::pymodule;
use sqlbuild_rules::constants::{API_VERSION, NATIVE_BUILD_IDENTITY};

use crate::bindings::_helpers::analysis;
use crate::bindings::_helpers::analysis_session;
use crate::bindings::_helpers::attachments;
use crate::bindings::_helpers::auditing;
use crate::bindings::_helpers::authored_values;
use crate::bindings::_helpers::cache;
use crate::bindings::_helpers::cli;
use crate::bindings::_helpers::compile_attachment;
use crate::bindings::_helpers::compile_command;
use crate::bindings::_helpers::compile_driver;
use crate::bindings::_helpers::compile_outputs;
use crate::bindings::_helpers::compiled_project;
use crate::bindings::_helpers::contracts;
use crate::bindings::_helpers::cursor;
use crate::bindings::_helpers::dag_artifact;
use crate::bindings::_helpers::diagnostics;
use crate::bindings::_helpers::discovery;
use crate::bindings::_helpers::exported_constants;
use crate::bindings::_helpers::fingerprints;
use crate::bindings::_helpers::frontier;
use crate::bindings::_helpers::graph;
use crate::bindings::_helpers::lifecycle_events;
use crate::bindings::_helpers::lineage;
use crate::bindings::_helpers::macro_bridge;
use crate::bindings::_helpers::manifest;
use crate::bindings::_helpers::microbatches;
use crate::bindings::_helpers::model_config;
use crate::bindings::_helpers::native_host;
use crate::bindings::_helpers::plan_output;
use crate::bindings::_helpers::planner_registration;
use crate::bindings::_helpers::project_assembly;
use crate::bindings::_helpers::record_behaviours;
use crate::bindings::_helpers::refactor;
use crate::bindings::_helpers::references;
use crate::bindings::_helpers::render;
use crate::bindings::_helpers::rules;
use crate::bindings::_helpers::scopes;
use crate::bindings::_helpers::semantic_checks;
use crate::bindings::_helpers::semantic_shapes;
use crate::bindings::_helpers::sql_test_glue;
use crate::bindings::_helpers::sqltext;
use crate::bindings::_helpers::state_rows;
use crate::bindings::_helpers::type_system;
use sqlbuild_python_base::boundary::main::register_expected_errors::register_expected_errors;
use sqlbuild_python_base::boundary::main::register_native_compiler_error::register_native_compiler_error;
use sqlbuild_python_base::boundary::main::register_oracles::register_oracles;
use sqlbuild_python_base::records::main::registry::capture::capture;
use sqlbuild_python_base::records::main::registry::register_records::register_records;
use sqlbuild_python_base::records::main::registry::verify_attributes::verify_attributes;
use sqlbuild_python_base::records::main::registry::verify_tables::verify_tables;
use sqlbuild_python_project::analysis::main::register_normalization::register_normalization;
use sqlbuild_python_project::analysis::models::BindingPositions;
use sqlbuild_python_project::analysis::models::CompactAnalysisJob;
use sqlbuild_python_project::analysis::models::ProjectCatalog;

#[pymodule]
pub fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_records(
        module,
        record_behaviours::constants::GETTER_TABLES,
        &record_behaviours::constants::RECORD_BEHAVIOURS,
    )?;
    verify_tables()?;
    capture(module)?;
    native_host::registration::register(module)?;
    rules::registration::register(module)?;
    cli::registration::register(module)?;
    register_normalization(module)?;
    analysis::queries::register(module)?;
    analysis::sql_tests::register(module)?;
    sqltext::registration::register(module)?;
    register_oracles(module)?;
    register_expected_errors(module)?;
    discovery::registration::register(module)?;
    scopes::registration::register(module)?;
    model_config::registration::register(module)?;
    render::registration::register(module)?;
    register_project_stages(module)?;
    register_analysis_stages(module)?;
    module.add("API_VERSION", API_VERSION)?;
    module.add("BUILD_IDENTITY", NATIVE_BUILD_IDENTITY)?;
    verify_attributes(module)
}

fn register_project_stages(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_native_compiler_error(module)?;
    module.add_class::<ProjectCatalog>()?;
    module.add_class::<CompactAnalysisJob>()?;
    module.add_class::<BindingPositions>()?;
    cache::registration::register(module)?;
    attachments::registration::register(module)?;
    refactor::registration::register(module)?;
    plan_output::registration::register(module)?;
    planner_registration::registration::register(module)?;
    references::registration::register(module)?;
    authored_values::registration::register(module)?;
    auditing::registration::register(module)?;
    cursor::registration::register(module)?;
    exported_constants::registration::register(module)?;
    state_rows::registration::register(module)?;
    microbatches::registration::register(module)?;
    lifecycle_events::registration::register(module)?;
    diagnostics::registration::register(module)?;
    analysis::sql_test_comparisons::register(module)?;
    Ok(())
}

fn register_analysis_stages(module: &Bound<'_, PyModule>) -> PyResult<()> {
    type_system::registration::register(module)?;
    analysis_session::registration::register(module)?;
    semantic_checks::registration::register(module)?;
    semantic_shapes::registration::register(module)?;
    contracts::registration::register(module)?;
    lineage::registration::register(module)?;
    fingerprints::registration::register(module)?;
    sql_test_glue::registration::register(module)?;
    project_assembly::registration::register(module)?;
    compile_outputs::registration::register(module)?;
    graph::registration::register(module)?;
    compiled_project::registration::register(module)?;
    compile_attachment::registration::register(module)?;
    compile_command::registration::register(module)?;
    compile_driver::registration::register(module)?;
    frontier::registration::register(module)?;
    macro_bridge::registration::register(module)?;
    dag_artifact::registration::register(module)?;
    manifest::registration::register(module)?;
    Ok(())
}
