//! The connections a build that names limited jobs opens.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

use crate::compile_driver::_helpers::jobs::job_limits::job_limit_total;
use crate::compile_driver::main::jobs::build_concurrency::build_concurrency;
use sqlbuild_config::effective::models::EffectiveConcurrency;

/// The build's limit, or the named jobs' limits added together when every planned node has one.
pub fn connection_limit(
    configured: EffectiveConcurrency,
    pipeline: &Bound<'_, PyAny>,
) -> PyResult<i64> {
    let limit: i64 = build_concurrency(configured, pipeline)?.limit;
    let Some(total) = job_limit_total(pipeline)? else {
        return Ok(limit);
    };
    let node_jobs: Bound<'_, PyDict> = native_field(pipeline, "node_jobs")?.cast_into()?;
    for key in
        native_field(&native_field(pipeline, "plan_output")?, "execution_order")?.try_iter()?
    {
        if !node_jobs.contains(key?)? {
            return Ok(limit);
        }
    }
    Ok(limit.min(total.max(1)))
}
