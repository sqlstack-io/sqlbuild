use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::pymodule;
use sqlbuild_rules::constants::API_VERSION;

use crate::bindings::_helpers::{
    analysis_session, attachments, auditing, authored_values, cache, cli, compile_attachment,
    compile_command, compile_driver, compile_outputs, compiled_project, contracts, cursor,
    dag_artifact, diagnostics, discovery, exported_constants, fingerprints, frontier, graph,
    lifecycle_events, lineage, macro_bridge, manifest, microbatches, model_config, native_host,
    plan_output, planner_registration, project_assembly, record_behaviours, refactor, references,
    render, rules, scopes, semantic_checks, semantic_shapes, sql_test_glue, sqltext, state_rows,
    type_system,
};
use sqlbuild_python_base::boundary::main::register_expected_errors::register_expected_errors;
use sqlbuild_python_base::boundary::main::register_native_compiler_error::register_native_compiler_error;
use sqlbuild_python_base::boundary::main::register_oracles::register_oracles;
use sqlbuild_python_base::records::main::registry::capture::capture;
use sqlbuild_python_base::records::main::registry::register_records::register_records;
use sqlbuild_python_base::records::main::registry::verify_attributes::verify_attributes;
use sqlbuild_python_base::records::main::registry::verify_tables::verify_tables;
use sqlbuild_python_compile::analysis::main::register_queries::register_queries;
use sqlbuild_python_compile::analysis::main::register_sql_test_comparisons::register_sql_test_comparisons;
use sqlbuild_python_compile::analysis::main::register_sql_tests::register_sql_tests;
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
    register_queries(module)?;
    register_sql_tests(module)?;
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
    register_sql_test_comparisons(module)?;
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
