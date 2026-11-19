//! The plan and pipeline fields the named jobs' settings give a planned build.

use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

use crate::compile_driver::models::PlannedJobSettings;
use crate::compile_driver::types::PipelineFields;

/// The plan with the jobs' settings applied, and their limits and warehouses as pipeline fields.
pub fn job_plan_fields<'py>(
    job_settings: Option<&PlannedJobSettings>,
    plan_output: Bound<'py, PyAny>,
    options: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, PipelineFields<'py>)> {
    let Some(job_settings) = job_settings else {
        return Ok((plan_output, Vec::new()));
    };
    let plan_output: Bound<'py, PyAny> = job_settings.apply_to_plan(
        &plan_output,
        &native_field(options, "cli_ignore_audit_failures")?,
    )?;
    let (limits, node_jobs) =
        job_settings.job_limits(&plan_output, &native_field(options, "concurrency")?)?;
    let (nodes, python) = job_settings.node_warehouses(&plan_output)?;
    Ok((
        plan_output,
        vec![
            ("job_limits", limits.into_any()),
            ("node_jobs", node_jobs.into_any()),
            ("node_warehouses", nodes.into_any()),
            ("python_node_warehouses", python.into_any()),
        ],
    ))
}
