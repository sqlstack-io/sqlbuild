//! The plan metadata's named jobs and the warehouses their nodes run on.

use crate::plan_output::_helpers::plan_text;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// The plan metadata's named jobs and the warehouses their nodes run on.
#[inline]
pub fn job_warehouses(metadata: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    plan_text::job_warehouses(metadata)
}
