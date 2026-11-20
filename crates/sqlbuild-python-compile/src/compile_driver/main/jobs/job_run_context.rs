//! What a build's run context says about its named jobs.

use crate::compile_driver::types::JobRunContext;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_project::compiled_project::constants::{
    JOB_FULL_REFRESH_KEY, JOB_WAREHOUSES_KEY,
};

/// The jobs whose models the planned build rebuilds, and each job's warehouse its nodes run on.
pub fn job_run_context(pipeline: &Bound<'_, PyAny>) -> PyResult<JobRunContext> {
    let metadata: Bound<'_, PyAny> =
        native_field(&native_field(pipeline, "plan_output")?, "metadata")?;
    let refreshing: Bound<'_, PyAny> = mapping_get(&metadata, JOB_FULL_REFRESH_KEY)?;
    let jobs: Vec<String> = if refreshing.is_none() {
        Vec::new()
    } else {
        refreshing
            .try_iter()?
            .map(|job| job?.extract())
            .collect::<PyResult<_>>()?
    };
    let warehouses: Bound<'_, PyAny> = mapping_get(&metadata, JOB_WAREHOUSES_KEY)?;
    let warehouses: Vec<(String, String)> = match warehouses.cast::<PyDict>() {
        Ok(warehouses) => warehouses
            .iter()
            .map(|(job, warehouse)| Ok((job.extract()?, warehouse.extract()?)))
            .collect::<PyResult<_>>()?,
        Err(_) => Vec::new(),
    };
    Ok((jobs, warehouses))
}
