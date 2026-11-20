//! The settings each planned node takes from the jobs its selection names.

use pyo3::prelude::{Bound, PyAny, PyResult};

use crate::compile_driver::_helpers::jobs::job_node_settings;
use crate::compile_driver::models::PlannedJobSettings;

/// The named jobs' settings per selected node; `None` when ignored, unnamed or unselectable.
#[inline]
pub fn planned_job_settings(
    inputs: (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    options: (&Bound<'_, PyAny>, bool),
) -> PyResult<Option<PlannedJobSettings>> {
    job_node_settings::planned_job_settings(inputs, options)
}
