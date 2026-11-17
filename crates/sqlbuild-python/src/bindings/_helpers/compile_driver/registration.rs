//! Register the native compile command driver.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_base::compile_context::main::diagnostic_collector::register_diagnostic_collector::register_diagnostic_collector;
use sqlbuild_python_base::compile_context::main::input_reads::register_input_reads::register_input_reads;
use crate::bindings::_helpers::compile_driver::{plan_pipeline, plan_result, static_context};
use sqlbuild_python_compile::compile_driver::main::stages::register_analysis::register_analysis;
use sqlbuild_python_compile::compile_driver::main::stages::register_analysis_cache::register_analysis_cache;
use sqlbuild_python_compile::compile_driver::main::stages::register_assembly::register_assembly;
use sqlbuild_python_compile::compile_driver::main::stages::register_command::register_command;
use sqlbuild_python_compile::compile_driver::main::stages::register_deferral::register_deferral;
use sqlbuild_python_compile::compile_driver::main::stages::register_diff::register_diff;
use sqlbuild_python_compile::compile_driver::main::stages::register_discovery::register_discovery;
use sqlbuild_python_compile::compile_driver::main::setup::register_inputs::register_inputs;
use sqlbuild_python_compile::compile_driver::main::setup::register_pipeline::register_pipeline;
use sqlbuild_python_compile::compile_driver::main::setup::register_pipeline_entries::register_pipeline_entries;
use sqlbuild_python_compile::compile_driver::main::setup::register_project::register_project;
use sqlbuild_python_compile::compile_driver::main::setup::register_python_plan_entries::register_python_plan_entries;
use sqlbuild_python_compile::compile_driver::main::setup::register_relation_targets::register_relation_targets;
use sqlbuild_python_compile::compile_driver::main::stages::register_selection_phases::register_selection_phases;
use sqlbuild_python_compile::compile_driver::main::setup::register_targets::register_targets;
use sqlbuild_python_base::compile_driver::main::register_stage_work::register_stage_work;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_analysis(module)?;
    register_analysis_cache(module)?;
    register_assembly(module)?;
    register_command(module)?;
    register_project(module)?;
    register_inputs(module)?;
    register_input_reads(module)?;
    register_discovery(module)?;
    register_pipeline(module)?;
    plan_pipeline::register(module)?;
    register_pipeline_entries(module)?;
    plan_result::register(module)?;
    register_deferral(module)?;
    register_diagnostic_collector(module)?;
    register_diff(module)?;
    register_python_plan_entries(module)?;
    register_relation_targets(module)?;
    register_selection_phases(module)?;
    static_context::register(module)?;
    register_stage_work(module)?;
    register_targets(module)
}
