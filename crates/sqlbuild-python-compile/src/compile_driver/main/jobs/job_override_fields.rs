//! The planner overrides the named jobs' settings add.

use pyo3::prelude::{PyResult, Python};

use crate::compile_driver::models::PlannedJobSettings;
use crate::compile_driver::types::PipelineFields;

/// The models and functions the named jobs rebuild, as the planner's `full_refresh_*` overrides.
pub fn job_override_fields<'py>(
    py: Python<'py>,
    job_settings: Option<&PlannedJobSettings>,
) -> PyResult<PipelineFields<'py>> {
    let Some(job_settings) = job_settings else {
        return Ok(Vec::new());
    };
    Ok(vec![
        (
            "full_refresh_models",
            job_settings.full_refresh_models(py)?.into_any(),
        ),
        (
            "full_refresh_functions",
            job_settings.full_refresh_functions(py)?.into_any(),
        ),
    ])
}
