//! The plan and pipeline fields the named jobs' settings give a planned build.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::job_settings::main::setting_scope::setting_scope;
use sqlbuild_analysis::job_settings::models::SettingScope;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::job_settings::main::given_settings::given_settings;

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
        (
            &native_field(options, "cli_ignore_audit_failures")?,
            native_field(options, "ignore_audit_failures")?.is_truthy()?,
        ),
    )?;
    let (limits, node_jobs) = job_settings.job_limits(
        &plan_output,
        given_settings()
            .iter()
            .any(|name| setting_scope(name) == SettingScope::JobLimit),
    )?;
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
