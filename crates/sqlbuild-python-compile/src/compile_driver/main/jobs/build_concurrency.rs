//! The concurrency limit of a build that names jobs.

use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_config::effective::models::EffectiveConcurrency;

use crate::compile_driver::_helpers::jobs::job_limits::job_limit_total;

/// The build's limit: the configured one, raised to the named jobs' limits added together.
pub fn build_concurrency(
    configured: EffectiveConcurrency,
    pipeline: &Bound<'_, PyAny>,
) -> PyResult<EffectiveConcurrency> {
    Ok(match job_limit_total(pipeline)? {
        Some(total) if total > configured.limit => EffectiveConcurrency { limit: total },
        _ => configured,
    })
}
