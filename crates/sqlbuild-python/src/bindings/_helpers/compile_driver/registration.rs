//! Register the native compile command driver.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_base::compile_context::main::diagnostic_collector::register_diagnostic_collector::register_diagnostic_collector;
use sqlbuild_python_base::compile_context::main::input_reads::register_input_reads::register_input_reads;
use crate::bindings::_helpers::compile_driver::{analysis, analysis_cache, assembly, command, deferral, diff, discovery, inputs, pipeline, pipeline_entries, plan_pipeline, plan_result, project, python_plan_entries, relation_targets, selection_phases, static_context, targets};
use sqlbuild_python_base::compile_driver::main::register_stage_work::register_stage_work;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    analysis::register(module)?;
    analysis_cache::register(module)?;
    assembly::register(module)?;
    command::register(module)?;
    project::register(module)?;
    inputs::register(module)?;
    register_input_reads(module)?;
    discovery::register(module)?;
    pipeline::register(module)?;
    plan_pipeline::register(module)?;
    pipeline_entries::register(module)?;
    plan_result::register(module)?;
    deferral::register(module)?;
    register_diagnostic_collector(module)?;
    diff::register(module)?;
    python_plan_entries::register(module)?;
    relation_targets::register(module)?;
    selection_phases::register(module)?;
    static_context::register(module)?;
    register_stage_work(module)?;
    targets::register(module)
}
