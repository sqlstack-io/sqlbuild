//! Register the progress reporters functions and classes.

use crate::build_progress::models::ConnectionProgressReporter;
use crate::build_progress::models::DbtInitProgressReporter;
use crate::build_progress::models::PlanningProgressReporter;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the progress reporters functions and classes.
pub fn register_progress_reporters(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<ConnectionProgressReporter>()?;
    module.add_class::<DbtInitProgressReporter>()?;
    module.add_class::<PlanningProgressReporter>()
}
