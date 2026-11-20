//! The named jobs' limits a planned build carries.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The named jobs' `concurrency` limits added together; none without a command-line override.
pub(crate) fn job_limit_total(pipeline: &Bound<'_, PyAny>) -> PyResult<Option<i64>> {
    let limits: Bound<'_, PyDict> = native_field(pipeline, "job_limits")?.cast_into()?;
    if limits.is_empty() {
        return Ok(None);
    }
    let mut total: i64 = 0;
    for value in limits.values() {
        total = total.saturating_add(value.extract::<i64>()?);
    }
    Ok(Some(total))
}
